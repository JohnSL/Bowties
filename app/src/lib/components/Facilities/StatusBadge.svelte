<script lang="ts">
  /**
   * StatusBadge — the one status chip every facility card uses (Wired, Incomplete,
   * Waiting, Empty …). The caller chooses the text and tone; the badge only draws it.
   */
  let { text, tone = 'neutral' }: { text: string; tone?: 'neutral' | 'incomplete' | 'wired' } = $props();
</script>

<span class="status-badge {tone}" data-testid="status-badge">{text}</span>

<style>
  .status-badge {
    font-size: 0.6875rem;
    font-weight: 600;
    padding: 0.125rem 0.5rem;
    border-radius: 10px;
    white-space: nowrap;
  }
  .status-badge.neutral {
    background: #ececec;
    color: #424242;
  }
  .status-badge.incomplete {
    background: #fff4ce;
    color: #bc4b09;
  }
  .status-badge.wired {
    background: #dcfce7;
    color: #166534;
  }
</style>
