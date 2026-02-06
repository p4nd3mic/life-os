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
fn test_build_causal_content_claim_response_groups_keep_titles_cohesive() {
    let enriched = EnrichedData {
        title: "Cowboy Bebop structure".to_string(),
        subtitle: None,
        summary: None,
        stats: None,
        entities: None,
        image: None,
        expanded: Some(ExpandedContent {
            original_input: Some("Episode 5 should be first".to_string()),
            sections: vec![ExpandedSection {
                title: "Codex Response".to_string(),
                body: "## It should've been Episode 1 — I get it, but here's the tradeoff\nYour instinct makes sense because Ep 5 is the strongest hook for the main arc.\nIf it was Episode 1, you'd gain:\n- immediate main-plot momentum\n- clear antagonist framing\nBut you'd lose:\n- tonal setup and surprise reveal\n\n## If you wanted a better order\nEp 1 > Ep 2 > Ep 5\n- keeps onboarding vibe\n- gets to the spine faster".to_string(),
            }],
            entity_links: None,
            actions: vec![],
        }),
        image_lookup: None,
    };

    let causal = build_causal_content(
        "card-5",
        &CardType::Thought,
        "Episode 5 should be first",
        "2026-02-06T01:14:00",
        &enriched,
    );

    assert_eq!(causal.right_nodes.len(), 2);
    assert!(causal.right_nodes[0]
        .title
        .as_deref()
        .unwrap_or_default()
        .to_lowercase()
        .contains("tradeoff"));
    assert!(causal.right_nodes[0]
        .bullets
        .as_ref()
        .map(|items| !items.is_empty())
        .unwrap_or(false));
    assert!(causal.right_nodes[1]
        .title
        .as_deref()
        .unwrap_or_default()
        .to_lowercase()
        .contains("better order"));
}

#[test]
fn test_build_causal_content_claim_response_preface_attaches_to_first_heading() {
    let enriched = EnrichedData {
        title: "Cowboy Bebop structure".to_string(),
        subtitle: None,
        summary: None,
        stats: None,
        entities: None,
        image: None,
        expanded: Some(ExpandedContent {
            original_input: Some("Episode 5 should be first".to_string()),
            sections: vec![ExpandedSection {
                title: "Codex Response".to_string(),
                body: "Quick note before sections.\n## Why Ep 5 works\n- Spike history threads in\n## Tradeoff if moved to Ep 1\n- stronger immediate arc".to_string(),
            }],
            entity_links: None,
            actions: vec![],
        }),
        image_lookup: None,
    };

    let causal = build_causal_content(
        "card-6",
        &CardType::Thought,
        "Episode 5 should be first",
        "2026-02-06T02:04:00",
        &enriched,
    );

    assert_eq!(causal.right_nodes.len(), 2);
    let first_details = causal.right_nodes[0]
        .details
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    assert!(first_details.contains("quick note before sections"));
    assert!(first_details.contains("spike history"));
}

#[test]
fn test_build_causal_content_rewrites_fragmentary_why_headline() {
    let enriched = EnrichedData {
        title: "Cowboy Bebop thought".to_string(),
        subtitle: None,
        summary: None,
        stats: None,
        entities: None,
        image: None,
        expanded: Some(ExpandedContent {
            original_input: Some("Episode 5 should be first".to_string()),
            sections: vec![ExpandedSection {
                title: "Codex Response".to_string(),
                body: "## If it was Episode 1, you'd gain:\n- immediate main-plot momentum\n- clear antagonist framing".to_string(),
            }],
            entity_links: None,
            actions: vec![],
        }),
        image_lookup: None,
    };

    let causal = build_causal_content(
        "card-6b",
        &CardType::Thought,
        "Episode 5 should be first",
        "2026-02-06T02:09:00",
        &enriched,
    );

    assert!(!causal.right_nodes.is_empty());
    let headline = causal.right_nodes[0]
        .headline
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    assert!(!headline.ends_with("you'd gain"));
    assert!(!headline.ends_with("you d gain"));
}

#[test]
fn test_build_causal_content_filters_meta_boilerplate_nodes() {
    let enriched = EnrichedData {
        title: "Cowboy Bebop thought".to_string(),
        subtitle: None,
        summary: None,
        stats: None,
        entities: None,
        image: None,
        expanded: Some(ExpandedContent {
            original_input: Some("Episode 5 should be first".to_string()),
            sections: vec![ExpandedSection {
                title: "Codex Response".to_string(),
                body: "JMWillis — yep, Episode 5 is that episode.\n## Why Ep 5 works\n- Spike history and Vicious pull the spine in\n## Tradeoff\n- you lose onboarding vibe if it starts too heavy".to_string(),
            }],
            entity_links: None,
            actions: vec![],
        }),
        image_lookup: None,
    };

    let causal = build_causal_content(
        "card-6c",
        &CardType::Thought,
        "Episode 5 should be first",
        "2026-02-06T02:10:00",
        &enriched,
    );

    assert!(causal
        .right_nodes
        .iter()
        .all(|node| !node
            .headline
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .contains("jmwillis")));
}

#[test]
fn test_build_causal_content_sets_statement_why_semantic_mode_and_compaction() {
    let enriched = EnrichedData {
        title: "Cowboy Bebop structure".to_string(),
        subtitle: None,
        summary: None,
        stats: None,
        entities: None,
        image: None,
        expanded: Some(ExpandedContent {
            original_input: Some("Episode 5 should be first".to_string()),
            sections: vec![ExpandedSection {
                title: "Codex Response".to_string(),
                body: "## Why Ep 5 feels like the real show starts here\n- reason one\n## Tradeoff\n- reason two\n## Better order\n- reason three\n## Why this hits emotionally\n- reason four".to_string(),
            }],
            entity_links: None,
            actions: vec![],
        }),
        image_lookup: None,
    };

    let causal = build_causal_content(
        "card-7",
        &CardType::Thought,
        "Episode 5 should be first",
        "2026-02-06T07:00:00",
        &enriched,
    );

    assert_eq!(
        causal.semantic_mode,
        Some(super::CausalSemanticMode::StatementWhy)
    );
    let compaction = causal.compaction.expect("compaction");
    assert!(compaction.enabled);
    assert_eq!(compaction.threshold, 3);
    assert_eq!(compaction.overflow_count, Some(1));
}

#[test]
fn test_build_causal_content_sets_question_response_mode_for_questions() {
    let enriched = EnrichedData {
        title: "Question card".to_string(),
        subtitle: None,
        summary: Some("Here is the answer.".to_string()),
        stats: None,
        entities: None,
        image: None,
        expanded: Some(ExpandedContent {
            original_input: Some("Why does episode 5 hit harder?".to_string()),
            sections: vec![],
            entity_links: None,
            actions: vec![],
        }),
        image_lookup: None,
    };

    let causal = build_causal_content(
        "card-8",
        &CardType::Generic,
        "Why does episode 5 hit harder?",
        "2026-02-06T08:00:00",
        &enriched,
    );

    assert_eq!(
        causal.semantic_mode,
        Some(super::CausalSemanticMode::QuestionResponse)
    );
    assert_eq!(causal.left_nodes[0].role, Some(CausalNodeRole::Question));
    assert_eq!(causal.right_nodes[0].role, Some(CausalNodeRole::Response));
}

#[test]
fn test_restructure_split_cause_splits_single_left_node() {
    let causal = CausalCardContent {
        left_nodes: vec![CausalNode {
            id: "card-x:left:0".to_string(),
            text: "Walked hard and watched Gundam.".to_string(),
            headline: None,
            summary_line: None,
            title: None,
            bullets: None,
            details: None,
            role: Some(CausalNodeRole::Cause),
            rank: None,
            group_type: None,
            is_image_applicable: true,
            image: None,
            entity: None,
            occurred_at: None,
        }],
        right_nodes: vec![CausalNode {
            id: "card-x:right:0".to_string(),
            text: "Felt amazing".to_string(),
            headline: None,
            summary_line: None,
            title: None,
            bullets: None,
            details: None,
            role: Some(CausalNodeRole::Effect),
            rank: None,
            group_type: None,
            is_image_applicable: false,
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
        semantic_mode: None,
        compaction: None,
        transcript_source: None,
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
            headline: None,
            summary_line: None,
            title: None,
            bullets: None,
            details: None,
            role: Some(CausalNodeRole::Action),
            rank: None,
            group_type: None,
            is_image_applicable: true,
            image: None,
            entity: None,
            occurred_at: None,
        }],
        right_nodes: vec![
            CausalNode {
                id: "card-y:right:0".to_string(),
                text: "Order #1".to_string(),
                headline: None,
                summary_line: None,
                title: None,
                bullets: None,
                details: None,
                role: Some(CausalNodeRole::Reward),
                rank: None,
                group_type: None,
                is_image_applicable: false,
                image: None,
                entity: None,
                occurred_at: None,
            },
            CausalNode {
                id: "card-y:right:1".to_string(),
                text: "Order #2".to_string(),
                headline: None,
                summary_line: None,
                title: None,
                bullets: None,
                details: None,
                role: Some(CausalNodeRole::Reward),
                rank: None,
                group_type: None,
                is_image_applicable: false,
                image: None,
                entity: None,
                occurred_at: None,
            },
        ],
        links: vec![],
        layout: None,
        semantic_mode: None,
        compaction: None,
        transcript_source: None,
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
            headline: None,
            summary_line: None,
            title: None,
            bullets: None,
            details: None,
            role: Some(CausalNodeRole::Cause),
            rank: None,
            group_type: None,
            is_image_applicable: true,
            image: None,
            entity: None,
            occurred_at: None,
        }],
        right_nodes: vec![CausalNode {
            id: "card-z:right:0".to_string(),
            text: "Energy boost".to_string(),
            headline: None,
            summary_line: None,
            title: None,
            bullets: None,
            details: None,
            role: Some(CausalNodeRole::Effect),
            rank: None,
            group_type: None,
            is_image_applicable: false,
            image: None,
            entity: None,
            occurred_at: None,
        }],
        links: vec![],
        layout: None,
        semantic_mode: None,
        compaction: None,
        transcript_source: None,
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
