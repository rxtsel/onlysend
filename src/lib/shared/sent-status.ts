/** Only terminal delivery problems get a badge; unknown events stay unobtrusive. */
export function sentFailureLabel(event: string | null | undefined): string | null {
  switch (event) {
    case "suppressed": return "Suppressed";
    case "bounced": return "Bounced";
    case "failed": return "Failed";
    case "canceled": return "Canceled";
    default: return null;
  }
}
