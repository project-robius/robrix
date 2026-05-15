<!-- Generated at Eastside Church of Christ from user prompts:
     "In message right click context, Add an option to Forward Message."
     "Fix Action feedback loop in Forward Message modal buttons, cancel and Submit." -->
spec: task
name: "Forward Messages"
inherits: project
tags: [month-1, messaging, high-priority, context-menu]
---

## Intent

Add a `Forward Message` option to the message right-click / long-press context menu so users can send an existing message to another Matrix room. The first implementation should be reliable and reviewable: it must open a forward modal, submit through the Matrix async request path, and close cleanly without modal/action feedback loops.

## Decisions

- UI trigger: message context menu opened by right-click or long-press.
- Menu label: `Forward Message`.
- The option is visible only for real message events with forwardable message content.
- Forwardable in v1 means text, notice, and emote message content. Media, location, verification, polls, stickers, encrypted attachment re-upload, and custom message types are out of scope unless they can be resent without re-upload.
- Forward flow: context menu emits `MessageAction::Forward`, `RoomScreen` extracts the latest effective message content, and the app opens a forward modal.
- Initial destination UI: modal accepts a Matrix destination room ID.
- Submit path: use `submit_async_request(MatrixRequest::ForwardMessage { ... })`; do not spawn raw Matrix tasks from UI code.
- Close semantics: cancel, Escape, and successful submit emit one modal close action; passive `ModalAction::Dismissed` only clears modal state and must not emit another close action.
- Forwarded content uses the latest effective message content so edited messages forward their edited content.
- Success and failure feedback are surfaced through toast notifications from the async forwarding path.

## Boundaries

### Allowed to Modify

- `src/home/new_message_context_menu.rs` - Add context-menu item, ability flag, and action emission.
- `src/home/room_screen.rs` - Handle `MessageAction::Forward` and prepare forward payload.
- `src/app.rs` - Host and open/close the forward modal.
- `src/shared/mod.rs` - Register the forward modal widget.
- `src/sliding_sync.rs` - Add or use `MatrixRequest::ForwardMessage`.
- `resources/i18n/en.json` - Add menu label.
- `resources/i18n/zh-CN.json` - Add menu label.

### Must Create

- `src/shared/forward_modal.rs` - Forward modal and forwarding result helpers.

### Optional

- `src/shared/room_selector.rs` - Reusable room selector helper for a later multi-room/search UI.

### Forbidden

- Do not forward reactions, receipts, read markers, or other message metadata.
- Do not show the forward option for non-message timeline items.
- Do not use raw tokio tasks from UI code for Matrix forwarding.
- Do not create an action feedback loop when closing or dismissing the forward modal.
- Do not run `cargo fmt` or `rustfmt`.

## Acceptance Criteria

Scenario: Forward option appears in message context menu
  Test:
    package: robrix
    filter: test_forward_menu
  Given user right-clicks or long-presses a forwardable message
  Then the message context menu appears
  And the "Forward Message" option is visible

Scenario: Forward option hidden for non-forwardable items
  Test:
    package: robrix
    filter: test_forward_menu_hidden_non_message
  Given user opens the context menu for a non-message timeline item
  Then the "Forward Message" option is not visible

Scenario: Forward modal opens from context menu
  Test:
    package: robrix
    filter: test_forward_modal_opens
  Given user opens the message context menu for a forwardable message
  When user selects "Forward Message"
  Then the context menu closes
  And the forward modal opens
  And the destination room ID input has keyboard focus

Scenario: Forward submit sends async Matrix request
  Test:
    package: robrix
    filter: test_forward_submit_request
  Given the forward modal is open for a message
  And user enters a valid destination room ID
  When user clicks Forward
  Then `MatrixRequest::ForwardMessage` is submitted via `submit_async_request`
  And the modal closes

Scenario: Forward submit preserves edited content
  Test:
    package: robrix
    filter: test_forward_uses_latest_effective_content
  Given the selected message has been edited
  When user forwards the message
  Then the forwarded content uses the latest effective message content
  And stale original content is not sent

Scenario: Invalid destination room ID stays in modal
  Test:
    package: robrix
    filter: test_forward_invalid_room_id
  Given the forward modal is open
  When user enters an invalid room ID
  And clicks Forward
  Then no Matrix request is submitted
  And the modal remains open
  And an inline validation error is shown

Scenario: Cancel closes modal once
  Test:
    package: robrix
    filter: test_forward_cancel_no_feedback_loop
  Given the forward modal is open
  When user clicks Cancel
  Then one `ForwardMessageModalAction::Close` action is emitted
  And the app closes the modal
  And no repeated close/dismiss action loop occurs

Scenario: Cancel abort the forward Message request
  Test:
    package: robrix
    filter: test_forward_cancel_no_feedback_loop
  Given the forward modal is open
  When user submit a forward cancel request and clicks Cancel
  Then one `ForwardMessageModalAction::Close` action is emitted,
  The Forward message request can be aborted,
  When the user clicks the cancel button,
  And the app closes the modal
  And no repeated close/dismiss action loop occurs

Scenario: Escape closes modal once
  Test:
    package: robrix
    filter: test_forward_escape_no_feedback_loop
  Given the forward modal is open
  When user presses Escape
  Then one `ForwardMessageModalAction::Close` action is emitted
  And the app closes the modal
  And no repeated close/dismiss action loop occurs

Scenario: Passive dismiss does not emit close action
  Test:
    package: robrix
    filter: test_forward_dismiss_no_feedback_loop
  Given the forward modal is open
  When the modal emits `ModalAction::Dismissed`
  Then the forward modal clears its pending message state
  And it does not emit `ForwardMessageModalAction::Close`
  And no feedback loop occurs

Scenario: Successful forward shows feedback
  Test:
    package: robrix
    filter: test_forward_success_feedback
  Given user forwards a message to one destination room
  When the async send succeeds
  Then a success toast is shown

Scenario: Forward failure shows feedback
  Test:
    package: robrix
    filter: test_forward_failure_feedback
  Level: integration
  Test Double: mock matrix-sdk send endpoint
  Targets: forward_modal, sliding_sync
  Given user forwards a message
  When the Matrix send fails
  Then a warning or error toast is shown
  And the source room is not affected

## Out of Scope

- Forwarding to users not in shared rooms.
- Batch forwarding multiple selected messages.
- Forwarding thread context as a thread.
- Forwarding reactions, receipts, redactions, or read markers.
- Full searchable room selector and multi-room checkbox UI beyond helper scaffolding.
- Media re-upload for encrypted media attachments.
