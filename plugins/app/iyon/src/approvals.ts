import type { ApprovalId, JsonValue } from "@iyon/sdk";
import { View } from "@iyon/tui";
import type { PendingApproval } from "./contracts.ts";

export class ApprovalStore {
  private readonly pending = new Map<string, PendingApproval>();

  request(approval: PendingApproval): void { this.pending.set(String(approval.approvalId), approval); }
  resolve(approvalId: ApprovalId | number): PendingApproval | undefined {
    const key = String(approvalId); const value = this.pending.get(key); this.pending.delete(key); return value;
  }
  get(approvalId: ApprovalId | number): PendingApproval | undefined { return this.pending.get(String(approvalId)); }
  values(): readonly PendingApproval[] { return [...this.pending.values()]; }
  clear(): void { this.pending.clear(); }
}

export function pendingApproval(approvalId: ApprovalId | number, toolCallId: string, toolName: string, argumentsValue: JsonValue): PendingApproval {
  return { approvalId, toolCallId, toolName, arguments: argumentsValue };
}

export function approvalView(approval: PendingApproval): View {
  return View.text(`Approve ${approval.toolName}? Press Enter to approve or Escape to reject.`).fillWidth();
}
