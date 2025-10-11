use std::collections::HashMap;
use crate::shared::membership_transitions::{generate_summary, UserEvent, TransitionType};

/// Example function showing how to use the membership_transitions module
/// with the room_screen summary_text functionality.
pub fn example_usage_for_summary_text() -> String {
    // This is an example of how you would collect UserEvent data
    // from actual Matrix timeline events and generate a summary
    let mut user_events: HashMap<String, Vec<UserEvent>> = HashMap::new();

    // Example: Alice joined and then left
    user_events.insert("alice".into(), vec![
        UserEvent { 
            user_id: "alice".into(), 
            display_name: "Alice".into(), 
            transition: TransitionType::Joined, 
            index: 0 
        },
        UserEvent { 
            user_id: "alice".into(), 
            display_name: "Alice".into(), 
            transition: TransitionType::Left, 
            index: 1 
        },
    ]);

    // Example: Bob changed avatar
    user_events.insert("bob".into(), vec![
        UserEvent { 
            user_id: "bob".into(), 
            display_name: "Bob".into(), 
            transition: TransitionType::ChangedAvatar, 
            index: 2 
        },
    ]);

    // Example: Charlie joined
    user_events.insert("charlie".into(), vec![
        UserEvent { 
            user_id: "charlie".into(), 
            display_name: "Charlie".into(), 
            transition: TransitionType::Joined, 
            index: 3 
        },
    ]);

    // Generate the summary text using the membership_transitions module
    // The second parameter (2) is the max number of names to show before "and N others"
    generate_summary(&user_events, 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_summary() {
        let summary = example_usage_for_summary_text();
        
        // The summary should contain the users and their actions
        assert!(summary.contains("Alice") || summary.contains("joined and left"));
        assert!(summary.contains("Bob") || summary.contains("changed avatar"));
        assert!(summary.contains("Charlie") || summary.contains("joined"));
        
        println!("Generated summary: {}", summary);
    }
}