use super::service::{
    apply_restructure_action, build_causal_content, detect_intent, truncate, EnrichedData,
};
use super::{
    CardState, CardType, CausalCardContent, CausalLayoutState, CausalLink, CausalNode,
    CausalNodeRole, CausalRestructureAction, DomainId, ExpandedContent, ExpandedSection,
    ImageStatus,
};

#[test]
fn test_truncate_ascii() {
    assert_eq!(truncate("hello world", 5), "hello");
    assert_eq!(truncate("hi", 10), "hi");
}

#[test]
fn test_truncate_unicode() {
    assert_eq!(truncate("今日は良い天気", 3), "今日は");
    assert_eq!(truncate("Hello 🌍🌎🌏", 8), "Hello 🌍🌎");
}

#[test]
fn test_truncate_empty() {
    assert_eq!(truncate("", 10), "");
}

#[test]
fn test_card_state_transitions() {
    assert!(CardState::Pending.can_transition_to(&CardState::Processing));
    assert!(CardState::Processing.can_transition_to(&CardState::Complete));
    assert!(CardState::Processing.can_transition_to(&CardState::Error));
    assert!(CardState::Processing.can_transition_to(&CardState::AwaitingInput));
    assert!(CardState::AwaitingInput.can_transition_to(&CardState::Processing));
    assert!(CardState::AwaitingInput.can_transition_to(&CardState::Cancelled));
    assert!(CardState::Error.can_transition_to(&CardState::Processing));

    assert!(!CardState::Complete.can_transition_to(&CardState::Pending));
    assert!(!CardState::Cancelled.can_transition_to(&CardState::Processing));
}

#[test]
fn test_domain_detection() {
    let (card_type, domain, _) = detect_intent("had eggs for breakfast");
    assert_eq!(card_type, CardType::Meal);
    assert_eq!(domain, DomainId::Nutrition);

    let (card_type, domain, _) = detect_intent("watching Alien tonight");
    assert_eq!(card_type, CardType::MediaAdd);
    assert_eq!(domain, DomainId::Media);

    let (card_type, domain, _) = detect_intent("took $15 order");
    assert_eq!(card_type, CardType::DeliveryOrder);
    assert_eq!(domain, DomainId::Delivery);

    let (card_type, domain, _) = detect_intent("thinking about life");
    assert_eq!(card_type, CardType::Thought);
    assert_eq!(domain, DomainId::General);
}

#[test]
fn test_build_causal_content_uses_input_and_summary() {
    let enriched = EnrichedData {
        title: "Gundam Wing thought".to_string(),
        subtitle: Some("Episode five should be first".to_string()),
        summary: Some("The show turns from vibes to plot.".to_string()),
        stats: None,
        entities: None,
        image: None,
        expanded: Some(ExpandedContent {
            original_input: Some("Episode 5 should have been first".to_string()),
            sections: vec![],
            entity_links: None,
            actions: vec![],
        }),
        image_lookup: None,
    };

    let causal = build_causal_content(
        "card-1",
        &CardType::Thought,
        "Episode five should be first",
        "2026-02-05T13:00:00",
        &enriched,
    );

    assert_eq!(causal.left_nodes.len(), 1);
    assert_eq!(causal.left_nodes[0].role, Some(CausalNodeRole::Cause));
    assert!(causal.left_nodes[0]
        .text
        .to_lowercase()
        .contains("episode five"));
    assert_eq!(causal.right_nodes.len(), 1);
    assert_eq!(causal.right_nodes[0].role, Some(CausalNodeRole::Response));
    assert!(causal.right_nodes[0]
        .text
        .to_lowercase()
        .contains("vibes to plot"));
    assert_eq!(causal.links.len(), 1);
}

#[test]
fn test_build_causal_content_extracts_completed_orders_rows() {
    let enriched = EnrichedData {
        title: "Delivery session".to_string(),
        subtitle: None,
        summary: None,
        stats: None,
        entities: None,
        image: Some(super::CardImage {
            url: None,
            status: ImageStatus::Ready,
            source: Some("test".to_string()),
        }),
        expanded: Some(ExpandedContent {
            original_input: Some("Dinner shift".to_string()),
            sections: vec![ExpandedSection {
                title: "Completed Orders".to_string(),
                body: "| Merchant | Payout | Miles |\n|---|---|---|\n| Panda Express | $12.50 | 4.0 |\n| Panera | $9.25 | 3.0 |".to_string(),
            }],
            entity_links: None,
            actions: vec![],
        }),
        image_lookup: None,
    };

    let causal = build_causal_content(
        "card-2",
        &CardType::DeliverySession,
        "Dinner shift",
        "2026-02-05T17:00:00",
        &enriched,
    );

    assert_eq!(causal.left_nodes.len(), 1);
    assert_eq!(causal.left_nodes[0].role, Some(CausalNodeRole::Action));
    assert_eq!(causal.right_nodes.len(), 2);
    assert_eq!(causal.right_nodes[0].role, Some(CausalNodeRole::Reward));
    assert!(causal.right_nodes[0].text.contains("Panda Express"));
    assert!(causal.right_nodes[1].text.contains("Panera"));
    assert!(causal.right_nodes[0].image.is_some());
    assert_eq!(causal.links.len(), 2);
}

#[test]
fn test_build_causal_content_uses_headers_for_claim_response() {
    let enriched = EnrichedData {
        title: "Bebop opinion".to_string(),
        subtitle: None,
        summary: None,
        stats: None,
        entities: None,
        image: None,
        expanded: Some(ExpandedContent {
            original_input: Some("Episode 5 should be first".to_string()),
            sections: vec![ExpandedSection {
                title: "Codex Response".to_string(),
                body: "## Why Ep 5 feels like the real show starts here\n- Bounties\n- Style\n- Jazz\n\n## It should've been Episode 1 — tradeoff\n- You gain momentum\n- You lose tonal setup".to_string(),
            }],
            entity_links: None,
            actions: vec![],
        }),
        image_lookup: None,
    };

    let causal = build_causal_content(
        "card-3",
        &CardType::Thought,
        "Episode 5 should be first",
        "2026-02-05T21:00:00",
        &enriched,
    );

    assert_eq!(causal.left_nodes[0].role, Some(CausalNodeRole::Cause));
    assert_eq!(causal.right_nodes.len(), 2);
    assert_eq!(causal.right_nodes[0].role, Some(CausalNodeRole::Response));
    assert!(causal.right_nodes[0]
        .text
        .to_lowercase()
        .contains("real show starts here"));
    assert!(causal.right_nodes[1]
        .text
        .to_lowercase()
        .contains("tradeoff"));
}

#[test]
fn test_build_causal_content_infers_claim_response_for_generic_with_headings() {
    let enriched = EnrichedData {
        title: "Response".to_string(),
        subtitle: None,
        summary: None,
        stats: None,
        entities: None,
        image: None,
        expanded: Some(ExpandedContent {
            original_input: Some("Cowboy Bebop ep 5 should be first".to_string()),
            sections: vec![ExpandedSection {
                title: "Codex Response".to_string(),
                body: "## Why Ep 5 feels like the real show starts here\n- bounties\n- style\n- jazz\n\n## It should've been Episode 1 — tradeoff".to_string(),
            }],
            entity_links: None,
            actions: vec![],
        }),
        image_lookup: None,
    };

    let causal = build_causal_content(
        "card-4",
        &CardType::Generic,
        "I think episode 5 should be first",
        "2026-02-05T22:32:00",
        &enriched,
    );

    assert_eq!(causal.left_nodes[0].role, Some(CausalNodeRole::Cause));
    assert_eq!(causal.right_nodes.len(), 2);
    assert_eq!(causal.right_nodes[0].role, Some(CausalNodeRole::Response));
    assert!(causal.right_nodes[0]
        .text
        .to_lowercase()
        .contains("real show starts here"));
    assert!(causal.right_nodes[1]
        .text
        .to_lowercase()
        .contains("tradeoff"));
}

#[test]
fn test_restructure_split_cause_splits_single_left_node() {
    let causal = CausalCardContent {
        left_nodes: vec![CausalNode {
            id: "card-x:left:0".to_string(),
            text: "Walked hard and watched Gundam.".to_string(),
            role: Some(CausalNodeRole::Cause),
            image: None,
            entity: None,
            occurred_at: None,
        }],
        right_nodes: vec![CausalNode {
            id: "card-x:right:0".to_string(),
            text: "Felt amazing".to_string(),
            role: Some(CausalNodeRole::Effect),
            image: None,
            entity: None,
            occurred_at: None,
        }],
        links: vec![CausalLink {
            id: Some("card-x:link:0".to_string()),
            from_id: "card-x:left:0".to_string(),
            to_id: "card-x:right:0".to_string(),
            label: None,
            strength: Some(1.0),
        }],
        layout: Some(CausalLayoutState {
            visible_right_count: Some(3),
            top_link_limit: Some(3),
            expanded: Some(false),
        }),
    };

    let next = apply_restructure_action(
        "card-x",
        &causal,
        CausalRestructureAction::SplitCause,
        &[],
        None,
    );

    assert!(next.left_nodes.len() >= 2);
    assert_eq!(next.links.len(), 1);
}

#[test]
fn test_restructure_merge_effects_reduces_right_nodes() {
    let causal = CausalCardContent {
        left_nodes: vec![CausalNode {
            id: "card-y:left:0".to_string(),
            text: "Delivery session".to_string(),
            role: Some(CausalNodeRole::Action),
            image: None,
            entity: None,
            occurred_at: None,
        }],
        right_nodes: vec![
            CausalNode {
                id: "card-y:right:0".to_string(),
                text: "Order #1".to_string(),
                role: Some(CausalNodeRole::Reward),
                image: None,
                entity: None,
                occurred_at: None,
            },
            CausalNode {
                id: "card-y:right:1".to_string(),
                text: "Order #2".to_string(),
                role: Some(CausalNodeRole::Reward),
                image: None,
                entity: None,
                occurred_at: None,
            },
        ],
        links: vec![],
        layout: None,
    };

    let next = apply_restructure_action(
        "card-y",
        &causal,
        CausalRestructureAction::MergeEffects,
        &[],
        None,
    );

    assert_eq!(next.right_nodes.len(), 1);
    assert!(next.right_nodes[0].text.contains("Order #1"));
}

#[test]
fn test_restructure_reframe_mode_maps_effect_to_reward() {
    let causal = CausalCardContent {
        left_nodes: vec![CausalNode {
            id: "card-z:left:0".to_string(),
            text: "Walk".to_string(),
            role: Some(CausalNodeRole::Cause),
            image: None,
            entity: None,
            occurred_at: None,
        }],
        right_nodes: vec![CausalNode {
            id: "card-z:right:0".to_string(),
            text: "Energy boost".to_string(),
            role: Some(CausalNodeRole::Effect),
            image: None,
            entity: None,
            occurred_at: None,
        }],
        links: vec![],
        layout: None,
    };

    let next = apply_restructure_action(
        "card-z",
        &causal,
        CausalRestructureAction::ReframeMode,
        &[],
        Some("action_reward"),
    );

    assert_eq!(next.left_nodes[0].role, Some(CausalNodeRole::Action));
    assert_eq!(next.right_nodes[0].role, Some(CausalNodeRole::Reward));
}
