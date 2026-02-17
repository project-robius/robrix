## 2024-05-23 - Icon-Only Buttons and Tooltips
**Learning:** Icon-only buttons (like in the navigation bar) often lack text labels for space efficiency, but this hurts accessibility and usability for new users. Adding tooltips is a crucial micro-UX improvement.
**Action:** Always check `NavigationTabBar` and similar component-dense areas for icon-only buttons and ensure they emit `TooltipAction::HoverIn`/`HoverOut` events. Use `cx.display_context.is_desktop()` to position tooltips correctly (Right for desktop sidebar, Top for mobile bottom bar).
