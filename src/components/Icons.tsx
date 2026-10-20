import type { ButtonHTMLAttributes } from "react";
export function NavigationIcon({ kind }: { kind: "home" | "notes" | "spaces" }) {
 return <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">{kind === "home" ? <><path d="M2 9 10 2l8 7"/><path d="M4 8v10h4v-6h4v6h4V8"/></> : kind === "spaces" ? <path d="M2 5h6l2 2h8v10H2z"/> : <><circle cx="10" cy="10" r="8"/><circle cx="10" cy="10" r="4" fill="currentColor" stroke="none"/></>}</svg>;
}
export function DeleteButton({ label, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
 return <button type="button" {...props} className={`delete-icon ${props.className || ""}`} aria-label={label} title={label}><svg width="18" height="18" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false"><path d="M3 5h14M7 5V2h6v3M5 5l1 13h8l1-13M8 8v7M12 8v7"/></svg></button>;
}
export function EditButton({ label, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
 return <button type="button" {...props} className={`edit-icon ${props.className || ""}`} aria-label={label} title={label}><svg width="16" height="16" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false"><path d="M12 20h7"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg></button>;
}
