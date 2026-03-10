## 2025-02-12 - Status Icon Tooltips
**Learning:** Status icons implemented as `View`s or `LoadingSpinner`s require manual hover detection in the parent widget's `handle_event` method, as they don't automatically support tooltips.
**Action:** Always check `visible()` before dispatching `TooltipAction::HoverIn` to avoid phantom tooltips for hidden elements.
