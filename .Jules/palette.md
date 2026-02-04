## 2024-05-23 - Makepad Tooltips Implementation
**Learning:** Tooltips in Makepad are not automatic properties of widgets. They must be implemented by handling `Hit::FingerHoverIn` and `Hit::FingerHoverOut` events in the widget's `handle_event` method and dispatching `TooltipAction::HoverIn` and `TooltipAction::HoverOut`.
**Action:** When adding tooltips to custom widgets, ensure the widget implements `handle_event` to intercept hover events and dispatch the appropriate actions to the global tooltip handler.
