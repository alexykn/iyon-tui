use iyon_core::{CoreEvent, MessageDelta, MessageRole, ToolCallDelta, ToolUpdateEvent};
use serde_json::{Value, json};
use crate::NativeError;

pub(crate) fn core_event(event: &CoreEvent) -> Value {
    match event {
        CoreEvent::AgentStarted => json!({"type": "agentStarted"}),
        CoreEvent::AgentFinished => json!({"type": "agentFinished"}),
        CoreEvent::TurnStarted { turn_id } => json!({"type": "turnStarted", "turnId": turn_id}),
        CoreEvent::SteerQueued { queue_id, text } => {
            json!({"type": "steerQueued", "queueId": queue_id, "text": text})
        }
        CoreEvent::MessageStarted {
            turn_id,
            message_id,
            role,
        } => json!({
            "type": "messageStarted", "turnId": turn_id, "messageId": message_id,
            "role": message_role(*role),
        }),
        CoreEvent::MessageDelta {
            turn_id,
            message_id,
            delta,
        } => json!({
            "type": "messageDelta", "turnId": turn_id, "messageId": message_id,
            "delta": message_delta(delta),
        }),
        CoreEvent::MessageFinished {
            turn_id,
            message_id,
        } => json!({
            "type": "messageFinished", "turnId": turn_id, "messageId": message_id,
        }),
        CoreEvent::ToolCallStarted {
            turn_id,
            message_id,
            tool_call_id,
            tool_name,
            arguments,
        } => json!({
            "type": "toolCallStarted", "turnId": turn_id, "messageId": message_id,
            "toolCallId": tool_call_id, "toolName": tool_name, "arguments": arguments,
        }),
        CoreEvent::ToolCallFinished {
            turn_id,
            message_id,
            tool_call_id,
            tool_name,
            is_error,
        } => json!({
            "type": "toolCallFinished", "turnId": turn_id, "messageId": message_id,
            "toolCallId": tool_call_id, "toolName": tool_name, "isError": is_error,
        }),
        CoreEvent::ToolCallUpdated {
            turn_id,
            message_id,
            tool_call_id,
            tool_name,
            update,
        } => json!({
            "type": "toolCallUpdated", "turnId": turn_id, "messageId": message_id,
            "toolCallId": tool_call_id, "toolName": tool_name, "update": tool_update(update),
        }),
        CoreEvent::ToolResultStarted {
            turn_id,
            message_id,
            tool_call_id,
            tool_name,
            is_error,
        } => json!({
            "type": "toolResultStarted", "turnId": turn_id, "messageId": message_id,
            "toolCallId": tool_call_id, "toolName": tool_name, "isError": is_error,
        }),
        CoreEvent::ToolResultFinished {
            turn_id,
            message_id,
            tool_call_id,
            tool_name,
            text,
            details,
            is_error,
        } => json!({
            "type": "toolResultFinished", "turnId": turn_id, "messageId": message_id,
            "toolCallId": tool_call_id, "toolName": tool_name, "text": text,
            "details": details, "isError": is_error,
        }),
        CoreEvent::ToolApprovalRequested {
            turn_id,
            approval_id,
            message_id,
            tool_call_id,
            tool_name,
            arguments,
        } => json!({
            "type": "toolApprovalRequested", "turnId": turn_id, "approvalId": approval_id,
            "messageId": message_id, "toolCallId": tool_call_id, "toolName": tool_name,
            "arguments": arguments,
        }),
        CoreEvent::ToolApprovalResolved {
            turn_id,
            approval_id,
            tool_call_id,
            approved,
            reason,
        } => json!({
            "type": "toolApprovalResolved", "turnId": turn_id, "approvalId": approval_id,
            "toolCallId": tool_call_id, "approved": approved, "reason": reason,
        }),
        CoreEvent::TurnFinished { turn_id } => json!({"type": "turnFinished", "turnId": turn_id}),
        CoreEvent::TurnFailed { turn_id, message } => json!({
            "type": "turnFailed", "turnId": turn_id, "message": message,
        }),
        CoreEvent::TurnCancelled { turn_id } => json!({"type": "turnCancelled", "turnId": turn_id}),
        CoreEvent::ConfigChanged {
            provider,
            model_id,
            reasoning_effort,
        } => json!({
            "type": "configChanged", "provider": provider, "modelId": model_id,
            "reasoningEffort": reasoning_effort.code(),
        }),
    }
}

fn message_role(role: MessageRole) -> &'static str {
    match role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::ToolResult => "toolResult",
        MessageRole::Status => "status",
    }
}

fn message_delta(delta: &MessageDelta) -> Value {
    match delta {
        MessageDelta::Text(text) => json!({"type": "text", "text": text}),
        MessageDelta::Thinking(text) => json!({"type": "thinking", "text": text}),
        MessageDelta::ToolCall(delta) => {
            json!({"type": "toolCall", "delta": tool_call_delta(delta)})
        }
    }
}

fn tool_call_delta(delta: &ToolCallDelta) -> Value {
    match delta {
        ToolCallDelta::Start {
            content_index,
            tool_call_id,
            tool_name,
        } => json!({
            "type": "start", "contentIndex": content_index, "toolCallId": tool_call_id,
            "toolName": tool_name,
        }),
        ToolCallDelta::Arguments {
            content_index,
            tool_call_id,
            tool_name,
            delta,
        } => json!({
            "type": "arguments", "contentIndex": content_index, "toolCallId": tool_call_id,
            "toolName": tool_name, "delta": delta,
        }),
        ToolCallDelta::End {
            content_index,
            tool_call_id,
            tool_name,
            arguments,
        } => json!({
            "type": "end", "contentIndex": content_index, "toolCallId": tool_call_id,
            "toolName": tool_name, "arguments": arguments,
        }),
    }
}

fn tool_update(update: &ToolUpdateEvent) -> Value {
    match update {
        ToolUpdateEvent::Text(text) => json!({"type": "text", "text": text}),
        ToolUpdateEvent::Progress {
            label,
            current,
            total,
        } => json!({
            "type": "progress", "label": label, "current": current, "total": total,
        }),
        ToolUpdateEvent::Details(details) => json!({"type": "details", "details": details}),
    }
}

pub(crate) fn parse_tool_update(value: Value) -> napi::Result<ToolUpdateEvent> {
    let object = value.as_object().ok_or_else(|| {
        NativeError::invalid_input("tool update must be an object")
    })?;
    let type_str = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| NativeError::invalid_input("tool update type must be a string"))?;
    match type_str {
        "text" => {
            let text = object
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    NativeError::invalid_input("text update requires a text field")
                })?;
            Ok(ToolUpdateEvent::Text(text.to_owned()))
        }
        "progress" => {
            let label = object
                .get("label")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    NativeError::invalid_input("progress update requires a label field")
                })?;
            let current = object.get("current").and_then(Value::as_u64);
            let total = object.get("total").and_then(Value::as_u64);
            Ok(ToolUpdateEvent::Progress {
                label: label.to_owned(),
                current,
                total,
            })
        }
        "details" => {
            let details = object
                .get("details")
                .ok_or_else(|| {
                    NativeError::invalid_input("details update requires a details field")
                })?
                .clone();
            Ok(ToolUpdateEvent::Details(details))
        }
        other => Err(NativeError::invalid_input(format!(
            "unknown tool update type `{other}`"
        ))),
    }
}
