## 2026-05-20 - [XSS vulnerability in VerificationRequest events]
 **Vulnerability:** Unescaped HTML injection in VerificationRequest message preview and display.
 **Learning:** User-controlled input in matrix events was directly interpolated into HTML strings via format!.
 **Prevention:** Ensure all user input is sanitized using htmlize::escape_text before being used in format strings passed to show_html.
