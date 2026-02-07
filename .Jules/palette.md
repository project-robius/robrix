## 2024-05-22 - Manual Tooltip Implementation for Icon Buttons
**Learning:** `RobrixIconButton` (and custom buttons in general) do not have built-in tooltip support. Tooltips must be implemented by manually catching `Hit::FingerHoverIn`/`Out` events and dispatching `TooltipAction`s.
**Action:** When adding icon-only buttons, always wrap them or handle their events to dispatch `TooltipAction` for accessibility and usability.
