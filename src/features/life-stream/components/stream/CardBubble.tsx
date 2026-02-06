import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  ClarificationOption,
  CausalRestructureAction,
  ImageCandidate,
  ImageCandidateResponse,
  StreamCard,
} from "../../types";
import { CardImage } from "./CardImage";
import { ExpandedCard } from "./ExpandedCard";
import { ProcessingIndicator } from "./ProcessingIndicator";
import { CauseEffectCard } from "./CauseEffectCard";
import { ImageAttachSheet, type ImageAttachSelectionOptions } from "./ImageAttachSheet";
import { resolveCardTitleWithIcon } from "../../utils/cardTitle";
import { getCardHighlights } from "../../utils/cardHighlights";
import { Markdown } from "../../../messages/components/Markdown";
import { buildCollapsedSummary } from "../../utils/summary";
import { resolveRenderableLayoutMode } from "../../utils/layoutMode";
import { useLifeStreamContextOptional } from "../../context/LifeStreamContext";
import "./CardBubble.css";

type ReviewCandidatesEventDetail = {
  cardId: string;
  nodeId?: string;
};

type CardBubbleProps = {
  card: StreamCard;
  onCancel: (cardId: string) => void;
  onRetry: (cardId: string) => void;
  onClarify: (cardId: string, optionId: string) => void;
  onRestructure: (
    cardId: string,
    action: CausalRestructureAction,
    options?: { sourceNodeIds?: string[]; targetMode?: "cause_effect" | "action_reward" },
  ) => void;
};

export function CardBubble({
  card,
  onCancel,
  onRetry,
  onClarify,
  onRestructure,
}: CardBubbleProps) {
  const context = useLifeStreamContextOptional();
  const [isExpanded, setIsExpanded] = useState(false);
  const [isImageAttachOpen, setIsImageAttachOpen] = useState(false);
  const [imageAttachTargetNodeId, setImageAttachTargetNodeId] = useState<string | null>(null);
  const [imageAttachContextHint, setImageAttachContextHint] = useState<string | null>(null);
  const [imageAttachResponse, setImageAttachResponse] = useState<ImageCandidateResponse | null>(null);
  const [imageAttachLoading, setImageAttachLoading] = useState(false);
  const [imageAttachError, setImageAttachError] = useState<string | null>(null);

  useEffect(() => {
    if (!card.expanded || card.state !== "complete") {
      setIsExpanded(false);
    }
  }, [card.expanded, card.state]);

  useEffect(() => {
    setImageAttachTargetNodeId(null);
    setImageAttachContextHint(null);
    setImageAttachResponse(null);
    setImageAttachError(null);
    setImageAttachLoading(false);
    setIsImageAttachOpen(false);
  }, [card.id]);

  const displayTitle = useMemo(() => resolveCardTitleWithIcon(card), [card]);
  const highlights = useMemo(() => getCardHighlights(card), [card]);
  const canCancel =
    card.state === "pending" ||
    card.state === "processing" ||
    card.state === "awaiting_input";
  const canRetry = card.state === "error";
  const canRetryMcp = card.state === "complete" && Boolean(card.errorMessage);
  const canExpand = card.state === "complete" && Boolean(card.expanded);
  const renderLayoutMode = useMemo(
    () => resolveRenderableLayoutMode(card),
    [card],
  );
  const showCauseEffectLayout =
    renderLayoutMode === "cause_effect" && Boolean(card.causal);
  const causalCard = showCauseEffectLayout ? card.causal : undefined;
  const isDeliverySession = card.cardType === "delivery_session";
  const clarificationOptions = card.clarificationOptions ?? [];
  const showPreview =
    !isDeliverySession &&
    Boolean(card.assistantPreview) &&
    !card.summary &&
    card.state !== "complete" &&
    card.state !== "error";
  const responseSection = useMemo(() => {
    const sections = card.expanded?.sections ?? [];
    return sections.find(
      (section) => section.title.toLowerCase() === "codex response",
    );
  }, [card.expanded]);
  const completedOrdersSection = useMemo(() => {
    const sections = card.expanded?.sections ?? [];
    return sections.find(
      (section) => section.title.toLowerCase() === "completed orders",
    );
  }, [card.expanded]);
  const totalsSection = useMemo(() => {
    const sections = card.expanded?.sections ?? [];
    return sections.find(
      (section) => section.title.toLowerCase() === "totals",
    );
  }, [card.expanded]);
  const asciiSummary = useMemo(() => {
    if (!isDeliverySession) return "";
    const summary = card.summary?.trim();
    if (summary) return summary;
    const response = responseSection?.body?.trim();
    if (response) return response;
    return card.assistantPreview?.trim() ?? "";
  }, [card.assistantPreview, card.summary, isDeliverySession, responseSection]);
  const deliveryBox = useMemo(() => {
    if (!isDeliverySession) return "";
    if (card.assistantPreview?.trim()) return card.assistantPreview.trim();
    return asciiSummary;
  }, [asciiSummary, card.assistantPreview, isDeliverySession]);
  const showDeliveryPreview = isDeliverySession && Boolean(deliveryBox);
  const summaryText = useMemo(() => {
    const response = responseSection?.body?.trim();
    if (response) return buildCollapsedSummary(response);
    const summary = card.summary?.trim();
    return summary ? buildCollapsedSummary(summary) : "";
  }, [card.summary, responseSection]);
  const inputText = card.originalInput?.trim();
  const requestBadges = useMemo(() => {
    const badges: Array<{ key: string; label: string; type: "model" | "effort" }> = [];
    if (card.request?.model) {
      badges.push({
        key: `model-${card.request.model}`,
        label: `Model: ${card.request.model}`,
        type: "model",
      });
    }
    if (card.request?.effort) {
      badges.push({
        key: `effort-${card.request.effort}`,
        label: `Effort: ${card.request.effort}`,
        type: "effort",
      });
    }
    return badges;
  }, [card.request]);

  const toggleExpanded = useCallback(() => {
    if (!canExpand) return;
    setIsExpanded((prev) => !prev);
  }, [canExpand]);

  const handleCardClick = useCallback(
    (event: React.MouseEvent<HTMLElement>) => {
      if (!canExpand) return;
      if (event.defaultPrevented) return;
      const selection = window.getSelection?.() ?? document.getSelection?.();
      if (selection && selection.toString().trim().length > 0) {
        return;
      }
      const target = event.target as HTMLElement | null;
      if (target?.closest("button, a, [data-no-toggle]")) {
        return;
      }
      toggleExpanded();
    },
    [canExpand, toggleExpanded],
  );

  const handleClarify = useCallback(
    (option: ClarificationOption) => {
      onClarify(card.id, option.id);
    },
    [card.id, onClarify],
  );

  const showImage =
    !showCauseEffectLayout &&
    Boolean(card.image) &&
    card.image?.status !== "missing";

  const closeImageAttachSheet = useCallback(() => {
    setIsImageAttachOpen(false);
    setImageAttachTargetNodeId(null);
    setImageAttachContextHint(null);
    setImageAttachResponse(null);
    setImageAttachError(null);
    setImageAttachLoading(false);
  }, []);

  const openImageAttachSheet = useCallback(
    async (nodeId: string | null, contextHint?: string | null) => {
      if (!context?.getImageCandidates) {
        setIsImageAttachOpen(true);
        setImageAttachTargetNodeId(nodeId);
        setImageAttachContextHint(contextHint ?? null);
        setImageAttachError("Image picker unavailable.");
        setImageAttachResponse(null);
        return;
      }

      setIsImageAttachOpen(true);
      setImageAttachTargetNodeId(nodeId);
      setImageAttachContextHint(contextHint ?? null);
      setImageAttachError(null);
      setImageAttachResponse(null);
      setImageAttachLoading(true);
      try {
        const response = await context.getImageCandidates(card.id, nodeId);
        if (!response) {
          setImageAttachError("No image candidates found.");
          return;
        }
        setImageAttachResponse(response);
      } catch (error) {
        setImageAttachError(error instanceof Error ? error.message : String(error));
      } finally {
        setImageAttachLoading(false);
      }
    },
    [card.id, context],
  );

  useEffect(() => {
    const handler = (event: Event) => {
      const customEvent = event as CustomEvent<ReviewCandidatesEventDetail>;
      const detail = customEvent.detail;
      if (!detail || detail.cardId !== card.id) {
        return;
      }
      const targetNodeId = detail.nodeId ?? null;
      const node = causalCard
        ? [...causalCard.leftNodes, ...causalCard.rightNodes].find(
            (item) => item.id === targetNodeId,
          )
        : undefined;
      void openImageAttachSheet(
        targetNodeId,
        node?.text ?? card.originalInput ?? null,
      );
    };
    window.addEventListener("life-stream-review-image-candidates", handler as EventListener);
    return () => {
      window.removeEventListener(
        "life-stream-review-image-candidates",
        handler as EventListener,
      );
    };
  }, [card.id, card.originalInput, causalCard, openImageAttachSheet]);

  const applyCandidateImage = useCallback(
    async (
      candidate: ImageCandidate,
      selectionOptions?: ImageAttachSelectionOptions,
    ) => {
      if (!context?.attachImage) {
        setImageAttachError("Attach image action unavailable.");
        return;
      }
      setImageAttachLoading(true);
      setImageAttachError(null);
      const result = await context.attachImage(card.id, candidate.sourcePath, {
        nodeId: imageAttachTargetNodeId,
        setPrimary: true,
        setContextOverride: selectionOptions?.setContextOverride ?? false,
        contextHint: imageAttachContextHint,
        updateEntityFile: selectionOptions?.updateEntityFile ?? true,
        updateEntityEmbed: selectionOptions?.updateEntityEmbed ?? false,
      });
      setImageAttachLoading(false);
      if (!result) {
        setImageAttachError("Failed to attach image.");
        return;
      }
      closeImageAttachSheet();
    },
    [
      card.id,
      closeImageAttachSheet,
      context,
      imageAttachContextHint,
      imageAttachTargetNodeId,
    ],
  );

  const browseForImage = useCallback(async () => {
    const { pickImageFiles } = await import("../../../../services/tauri");
    const selection = await pickImageFiles();
    const sourcePath = selection[0];
    if (!sourcePath) {
      return;
    }
    await applyCandidateImage({
      sourcePath,
      sourceKind: "manual_browse",
      score: 0,
      reason: ["manual-browse"],
      fileName: sourcePath.split("/").pop() ?? sourcePath,
      isManaged: false,
    });
  }, [applyCandidateImage]);

  return (
    <article
      className={`bubble life-card-bubble state-${card.state}${
        isExpanded ? " is-expanded" : ""
      } domain-${card.domain} layout-${renderLayoutMode}`}
      aria-expanded={isExpanded}
      onClick={handleCardClick}
    >
      <header className="life-card-bubble__header">
        <div className="life-card-bubble__meta">
          <div className="life-card-bubble__title">{displayTitle}</div>
          {card.subtitle && (
            <div className="life-card-bubble__subtitle">{card.subtitle}</div>
          )}
          {requestBadges.length > 0 && (
            <div className="life-card-bubble__badges">
              {requestBadges.map((badge) => (
                <span
                  key={badge.key}
                  className={`life-card-bubble__badge is-${badge.type}`}
                >
                  {badge.label}
                </span>
              ))}
            </div>
          )}
        </div>
        {canExpand && (
          <button
            type="button"
            className="life-card-bubble__expand"
            onClick={toggleExpanded}
            aria-label={isExpanded ? "Collapse card" : "Expand card"}
            aria-expanded={isExpanded}
          >
            {isExpanded ? "▼" : "▶"}
          </button>
        )}
      </header>

      {showPreview && (
        <div className="life-card-bubble__preview" data-no-toggle>
          <div className="life-card-bubble__preview-label">Live answer</div>
          <div className="life-card-bubble__preview-text">
            {card.assistantPreview}
          </div>
        </div>
      )}

      {showDeliveryPreview && (
        <div className="life-card-bubble__preview" data-no-toggle>
          <div className="life-card-bubble__preview-label">Live session</div>
          <pre className="life-card-bubble__ascii">{deliveryBox}</pre>
        </div>
      )}

      {inputText && isExpanded && (
        <div className="life-card-bubble__section">
          <div className="life-card-bubble__label">Input</div>
          <div className="life-card-bubble__text">{inputText}</div>
        </div>
      )}

      {causalCard && (
        <CauseEffectCard
          cardId={card.id}
          cardType={card.cardType}
          causal={causalCard}
          onRestructure={(action, options) => onRestructure(card.id, action, options)}
          onRequestNodeImage={(nodeId) => {
            const node = causalCard
              ? [...causalCard.leftNodes, ...causalCard.rightNodes].find(
                  (item) => item.id === nodeId,
                )
              : undefined;
            void openImageAttachSheet(nodeId, node?.text ?? card.originalInput ?? null);
          }}
        />
      )}

      {showImage && card.image && (
        <div className="life-card-bubble__image">
          <CardImage
            image={card.image}
            title={card.title}
            emoji={card.emoji}
            size={isExpanded ? "expanded" : "compact"}
            onRequestUpload={() => {
              void openImageAttachSheet(null, card.originalInput ?? null);
            }}
          />
        </div>
      )}

      <ImageAttachSheet
        open={isImageAttachOpen}
        loading={imageAttachLoading}
        error={imageAttachError}
        response={imageAttachResponse}
        contextHint={imageAttachContextHint}
        onClose={closeImageAttachSheet}
        onBrowse={() => {
          void browseForImage();
        }}
        onSelectCandidate={(candidate, options) => {
          void applyCandidateImage(candidate, options);
        }}
      />

      {highlights.length > 0 && (
        <div className="life-card-bubble__highlights">
          {highlights.map((item) => (
            <div
              key={`${item.label}-${item.value}`}
              className="life-card-bubble__highlight"
            >
              <span className="life-card-bubble__highlight-label">
                {item.label}
              </span>
              <span className="life-card-bubble__highlight-value">
                {item.value}
              </span>
            </div>
          ))}
        </div>
      )}

      {!showCauseEffectLayout && isDeliverySession && completedOrdersSection && (
        <div className="life-card-bubble__section" data-no-toggle>
          <div className="life-card-bubble__label">Completed Orders</div>
          <Markdown
            value={completedOrdersSection.body}
            className="markdown life-card-bubble__markdown"
          />
        </div>
      )}

      {!showCauseEffectLayout && isDeliverySession && totalsSection && (
        <div className="life-card-bubble__section" data-no-toggle>
          <div className="life-card-bubble__label">Totals</div>
          <Markdown
            value={totalsSection.body}
            className="markdown life-card-bubble__markdown"
          />
        </div>
      )}

      {summaryText && !isExpanded && !isDeliverySession && !showCauseEffectLayout && (
        <div className="life-card-bubble__section">
          <Markdown
            value={summaryText}
            className="markdown life-card-bubble__markdown"
          />
        </div>
      )}

      {card.stats && (
        <div className="life-card-bubble__stats">
          {Object.entries(card.stats).map(([key, value]) => (
            <span key={key} className="life-card-bubble__stat">
              {key}: {String(value)}
            </span>
          ))}
        </div>
      )}

      {card.entities && card.entities.length > 0 && (
        <div className="life-card-bubble__entities">
          {card.entities.map((entity) => (
            <span
              key={`${entity.type}-${entity.name}`}
              className="life-card-bubble__entity"
            >
              {entity.name}
            </span>
          ))}
        </div>
      )}

      {card.state === "awaiting_input" && clarificationOptions.length > 0 && (
        <div className="life-card-bubble__clarification">
          <div className="life-card-bubble__clarification-message">
            {card.processingStep ?? "Need more info"}
          </div>
          <div className="life-card-bubble__clarification-options">
            {clarificationOptions.map((option) => (
              <button
                key={option.id}
                type="button"
                className="life-card-bubble__clarification-option"
                onClick={() => handleClarify(option)}
              >
                {option.emoji && <span aria-hidden>{option.emoji}</span>}
                {option.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {card.errorMessage && (
        <div role="alert" className="life-card-bubble__error">
          {card.errorMessage}
        </div>
      )}

      {(canCancel || canRetry || canRetryMcp) && (
        <div className="life-card-bubble__actions">
          {canCancel && (
            <button
              type="button"
              className="life-card-bubble__action"
              onClick={() => onCancel(card.id)}
            >
              Cancel
            </button>
          )}
          {canRetry && (
            <button
              type="button"
              className="life-card-bubble__action is-primary"
              onClick={() => onRetry(card.id)}
            >
              Retry
            </button>
          )}
          {canRetryMcp && !canRetry && (
            <button
              type="button"
              className="life-card-bubble__action is-primary"
              onClick={() => onRetry(card.id)}
            >
              Retry tool
            </button>
          )}
        </div>
      )}

      <ProcessingIndicator card={card} />

      {isExpanded && canExpand && (
        <ExpandedCard card={card} onCollapse={() => setIsExpanded(false)} />
      )}
    </article>
  );
}
