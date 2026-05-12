## 2026-05-12 - Adding Tooltips to Icon-only Makepad Buttons
**Learning:** Tooltips for icon-only buttons like `RobrixIconButton` variants can be added by manually intercepting `Hit::FingerHoverIn` and `Hit::FingerHoverOut` in the parent view's `handle_event`, and dispatching `TooltipAction::HoverIn` / `HoverOut` actions to the UI context.
**Action:** When creating or modifying standalone icon buttons that lack built-in tooltip props, handle their hover area bounds manually using `cx.widget_action` with `CalloutTooltipOptions` to ensure accessibility.
