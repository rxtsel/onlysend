import { redirect } from "@sveltejs/kit";
import { mailUrl } from "$lib/features/auth/mail-routes";
import type { PageLoad } from "./$types";

export const load: PageLoad = ({ params }) => redirect(307, mailUrl(params.accountId, "sent"));
