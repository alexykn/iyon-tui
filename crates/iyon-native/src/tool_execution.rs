use std::sync::{Arc, Mutex as StdMutex};

use iyon_core::{
    CoreEvent, ToolUpdateEvent,
    ids::{ApprovalId, MessageId, ToolCallId, TurnId},
    kernel::{
        AgentMessage, ApprovalDecision, ApprovalRequirement, ApprovalState, ToolLifecycleEvent,
        ToolLifecycleHandle, ToolLifecycleResult, ToolLifecycleState,
    },
};
use napi::bindgen_prelude::Result;
use napi_derive::napi;
use serde_json::Value;

use crate::{NativeError, api, core::SessionState};

#[napi]
pub struct ToolExecution {
    state: Arc<SessionState>,
    turn_id: TurnId,
    message_id: MessageId,
    handle: StdMutex<Option<ToolLifecycleHandle>>,
}

impl ToolExecution {
    pub(crate) fn new(
        state: Arc<SessionState>,
        turn_id: TurnId,
        message_id: MessageId,
        tool_call_id: ToolCallId,
        tool_name: String,
        arguments: Value,
    ) -> Self {
        let call = iyon_core::kernel::AssembledToolCall {
            id: tool_call_id,
            name: tool_name,
            arguments,
        };
        Self {
            state,
            turn_id,
            message_id,
            handle: StdMutex::new(Some(ToolLifecycleHandle::new(call))),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Option<ToolLifecycleHandle>>> {
        self.handle
            .lock()
            .map_err(|_| NativeError::internal("tool execution lock is poisoned"))
    }

    fn with_handle<T>(
        &self,
        operation: impl FnOnce(&mut ToolLifecycleHandle) -> Result<T>,
    ) -> Result<T> {
        let mut handle = self.lock()?;
        operation(handle.as_mut().ok_or_else(NativeError::closed)?)
    }

    fn call_fields(&self) -> Result<(String, String, Value)> {
        let handle = self.lock()?;
        let call = handle.as_ref().ok_or_else(NativeError::closed)?.call();
        Ok((call.id.0.clone(), call.name.clone(), call.arguments.clone()))
    }

    fn approval_value(approval: &ApprovalState) -> Value {
        serde_json::json!({
            "id": approval.id.0,
            "requirement": approval_requirement_value(&approval.requirement),
            "status": approval_status_value(&approval.status),
        })
    }

    fn lifecycle_event_value(event: &ToolLifecycleEvent) -> Value {
        serde_json::json!({
            "sequence": event.sequence,
            "toolCallId": event.tool_call_id.0,
            "state": lifecycle_state_value(event.state),
            "approvalId": event.approval_id.map(|id| id.0),
        })
    }

    fn result_value(result: &ToolLifecycleResult) -> Value {
        serde_json::json!({
            "content": result.content.iter().map(crate::core::content_value).collect::<Vec<_>>(),
            "details": result.details,
            "isError": result.is_error,
        })
    }

    fn emit(&self, event: CoreEvent) -> Result<()> {
        self.state.try_emit(event)
    }
}

#[napi]
impl ToolExecution {
    #[napi]
    pub fn state(&self) -> Result<String> {
        self.with_handle(|handle| Ok(lifecycle_state_value(handle.state()).to_owned()))
    }

    #[napi]
    pub fn events(&self) -> Result<Vec<Value>> {
        self.with_handle(|handle| {
            Ok(handle
                .events()
                .iter()
                .map(Self::lifecycle_event_value)
                .collect())
        })
    }

    #[napi]
    pub fn prepared(&self, arguments: Value) -> Result<()> {
        self.state.ensure_open()?;
        self.with_handle(|handle| {
            handle
                .prepared(arguments)
                .map_err(|error| NativeError::invalid_input(error.to_string()))
        })
    }

    #[napi]
    pub fn start(&self) -> Result<()> {
        self.state.ensure_open()?;
        self.with_handle(|handle| {
            handle
                .start()
                .map_err(|error| NativeError::invalid_input(error.to_string()))
        })?;
        let (tool_call_id, tool_name, arguments) = self.call_fields()?;
        self.emit(CoreEvent::ToolCallStarted {
            turn_id: self.turn_id.0,
            message_id: self.message_id.0,
            tool_call_id: tool_call_id.clone(),
            tool_name: tool_name.clone(),
            arguments,
        })?;
        self.emit(CoreEvent::ToolResultStarted {
            turn_id: self.turn_id.0,
            message_id: self.message_id.0,
            tool_call_id,
            tool_name,
            is_error: false,
        })
    }

    #[napi(js_name = "requestApproval")]
    pub fn request_approval(&self, requirement: Option<Value>) -> Result<Option<Value>> {
        self.state.ensure_open()?;
        let requirement = crate::core::approval_requirement(requirement)?;
        let (approval, call) = {
            let mut handle = self.lock()?;
            let handle = handle.as_mut().ok_or_else(NativeError::closed)?;
            let approval_id = handle
                .request_approval(requirement)
                .map_err(|error| NativeError::invalid_input(error.to_string()))?;
            (approval_id, handle.call().clone())
        };
        let Some(approval_id) = approval else {
            return Ok(None);
        };
        self.emit(CoreEvent::ToolApprovalRequested {
            turn_id: self.turn_id.0,
            approval_id: approval_id.0,
            message_id: self.message_id.0,
            tool_call_id: call.id.0.clone(),
            tool_name: call.name.clone(),
            arguments: call.arguments.clone(),
        })?;
        let approval = self.with_handle(|handle| {
            handle
                .approval()
                .cloned()
                .ok_or_else(|| NativeError::internal("approval state disappeared"))
        })?;
        Ok(Some(Self::approval_value(&approval)))
    }

    #[napi]
    pub fn approve(&self, approval_id: f64) -> Result<()> {
        self.resolve_approval(approval_id, ApprovalDecision::Approved)
    }

    #[napi]
    pub fn reject(&self, approval_id: f64, reason: Option<String>) -> Result<()> {
        self.resolve_approval(approval_id, ApprovalDecision::Rejected { reason })
    }

    fn resolve_approval(&self, approval_id: f64, decision: ApprovalDecision) -> Result<()> {
        self.state.ensure_open()?;
        let approval_id = approval_id_to_native(approval_id)?;
        let (tool_call_id, approved, reason) = {
            let mut handle = self.lock()?;
            let handle = handle.as_mut().ok_or_else(NativeError::closed)?;
            handle
                .resolve_approval(approval_id, decision.clone())
                .map_err(|error| NativeError::invalid_input(error.to_string()))?;
            let call = handle.call();
            let (approved, reason) = match decision {
                ApprovalDecision::Approved => (true, None),
                ApprovalDecision::Rejected { reason } => (false, reason),
            };
            (call.id.0.clone(), approved, reason)
        };
        self.emit(CoreEvent::ToolApprovalResolved {
            turn_id: self.turn_id.0,
            approval_id: approval_id.0,
            tool_call_id,
            approved,
            reason,
        })
    }

    #[napi]
    pub fn finish(&self, value: Value) -> Result<()> {
        self.state.ensure_open()?;
        let object = crate::value::object(value, "tool result")?;
        let result = ToolLifecycleResult {
            content: crate::value::array(&object, "content")?
                .into_iter()
                .map(api::content_block)
                .collect::<Result<Vec<_>>>()?,
            details: object.get("details").cloned().unwrap_or(Value::Null),
            is_error: object
                .get("isError")
                .and_then(Value::as_bool)
                .ok_or_else(|| NativeError::invalid_input("isError must be a boolean"))?,
        };
        let result_value = Self::result_value(&result);
        self.with_handle(|handle| {
            handle
                .finish(result.clone())
                .map_err(|error| NativeError::invalid_input(error.to_string()))
        })?;
        let (tool_call_id, tool_name, _) = self.call_fields()?;
        self.state
            .session
            .lock()
            .map_err(|_| NativeError::internal("session lock is poisoned"))?
            .append_message(AgentMessage::tool_result(
                ToolCallId(tool_call_id.clone()),
                tool_name.clone(),
                result.content.clone(),
                result.details.clone(),
                result.is_error,
            ))
            .map_err(|error| NativeError::internal(error.to_string()))?;
        let text = result
            .content
            .iter()
            .filter_map(|block| match block {
                iyon_api::ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();
        self.emit(CoreEvent::ToolResultFinished {
            turn_id: self.turn_id.0,
            message_id: self.message_id.0,
            tool_call_id,
            tool_name,
            text,
            details: result_value.get("details").cloned().unwrap_or(Value::Null),
            is_error: result.is_error,
        })
    }

    #[napi]
    pub fn fail(&self, error: String) -> Result<()> {
        self.state.ensure_open()?;
        self.with_handle(|handle| {
            handle
                .fail(error.clone())
                .map_err(|error| NativeError::invalid_input(error.to_string()))
        })?;
        let (tool_call_id, tool_name, _) = self.call_fields()?;
        let details = serde_json::json!({});
        self.state
            .session
            .lock()
            .map_err(|_| NativeError::internal("session lock is poisoned"))?
            .append_message(AgentMessage::tool_result(
                ToolCallId(tool_call_id.clone()),
                tool_name.clone(),
                vec![iyon_api::ContentBlock::Text {
                    text: error.clone(),
                }],
                details.clone(),
                true,
            ))
            .map_err(|error| NativeError::internal(error.to_string()))?;
        self.emit(CoreEvent::ToolResultFinished {
            turn_id: self.turn_id.0,
            message_id: self.message_id.0,
            tool_call_id,
            tool_name,
            text: error,
            details,
            is_error: true,
        })
    }

    #[napi(js_name = "sendUpdate")]
    pub fn send_update(&self, update: Value) -> Result<()> {
        self.state.ensure_open()?;
        let tool_update = crate::events::parse_tool_update(update)?;
        let (tool_call_id, tool_name, _) = self.call_fields()?;
        self.emit(CoreEvent::ToolCallUpdated {
            turn_id: self.turn_id.0,
            message_id: self.message_id.0,
            tool_call_id,
            tool_name,
            update: tool_update,
        })
    }

    #[napi]
    pub fn cancel(&self, reason: Option<String>) -> Result<()> {
        self.state.ensure_open()?;
        self.with_handle(|handle| {
            handle
                .cancel(reason.unwrap_or_else(|| "cancelled".to_owned()))
                .map_err(|error| NativeError::invalid_input(error.to_string()))
        })
    }
}

fn approval_id_to_native(value: f64) -> Result<ApprovalId> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > u64::MAX as f64 {
        return Err(NativeError::invalid_input(
            "approval id must be a non-negative integer",
        ));
    }
    Ok(ApprovalId(value as u64))
}

fn lifecycle_state_value(state: ToolLifecycleState) -> &'static str {
    match state {
        ToolLifecycleState::Preparing => "preparing",
        ToolLifecycleState::Prepared => "prepared",
        ToolLifecycleState::Running => "running",
        ToolLifecycleState::PendingApproval => "pendingApproval",
        ToolLifecycleState::Finished => "finished",
        ToolLifecycleState::Failed => "failed",
        ToolLifecycleState::Cancelled => "cancelled",
    }
}

fn approval_requirement_value(requirement: &ApprovalRequirement) -> Value {
    match requirement {
        ApprovalRequirement::NotRequired => serde_json::json!({"type": "notRequired"}),
        ApprovalRequirement::Required { reason } => serde_json::json!({
            "type": "required", "reason": reason,
        }),
    }
}

fn approval_status_value(status: &iyon_core::kernel::ApprovalStatus) -> Value {
    match status {
        iyon_core::kernel::ApprovalStatus::Pending => serde_json::json!({"type": "pending"}),
        iyon_core::kernel::ApprovalStatus::Approved => serde_json::json!({"type": "approved"}),
        iyon_core::kernel::ApprovalStatus::Rejected { reason } => serde_json::json!({
            "type": "rejected", "reason": reason,
        }),
        iyon_core::kernel::ApprovalStatus::Cancelled => serde_json::json!({"type": "cancelled"}),
    }
}
