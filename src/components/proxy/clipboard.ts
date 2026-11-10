import { notifyFeedback } from "../ui/feedback";
export async function copyText(value: string | null) {
  if (!value) return;
  try {
    await navigator.clipboard.writeText(value);
    notifyFeedback("copied");
  } catch {
    notifyFeedback("copyFailed");
  }
}
