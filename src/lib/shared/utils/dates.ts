/** Formats Resend's ISO timestamps ("2023-04-03T22:13:42.674981+00:00")
 *  into compact relative labels ("14:32", "Yesterday", "3 days ago"). */
export function formatEmailDate(dateString: string): string {
  try {
    // Try parsing directly first
    let date: Date = new Date(dateString);

    // If invalid, try cleaning the string
    if (isNaN(date.getTime())) {
      // Remove microseconds if present (keep only milliseconds)
      const cleaned = dateString.replace(/(\.\d{3})\d+/, "$1");
      date = new Date(cleaned);
    }

    if (isNaN(date.getTime())) {
      console.warn("Invalid date format:", dateString);
      return dateString;
    }

    const now = new Date();
    const diffInMs = now.getTime() - date.getTime();
    const diffInDays = Math.floor(diffInMs / (1000 * 60 * 60 * 24));

    if (diffInDays === 0) {
      return date.toLocaleTimeString("en-US", {
        hour: "2-digit",
        minute: "2-digit",
      });
    } else if (diffInDays === 1) {
      return "Yesterday";
    } else if (diffInDays < 7) {
      return `${diffInDays} days ago`;
    } else if (diffInDays < 30) {
      const weeks = Math.floor(diffInDays / 7);
      return `${weeks} week${weeks > 1 ? "s" : ""} ago`;
    } else {
      return date.toLocaleDateString("en-US", {
        month: "short",
        day: "numeric",
        year: date.getFullYear() !== now.getFullYear() ? "numeric" : undefined,
      });
    }
  } catch (error) {
    console.error("Error formatting date:", dateString, error);
    return dateString;
  }
}
