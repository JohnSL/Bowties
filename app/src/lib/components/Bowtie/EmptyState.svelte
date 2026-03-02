<!--
  T021: EmptyState.svelte (US2)
  Shown in the Bowties tab when CDI reads complete but no shared event IDs were found.

  FR-006: centred illustration + "No connections yet" message + disabled "+ New Connection" CTA.
  FR-012: "+ New Connection" button is inert in this phase (future).
-->

<script lang="ts">
  // No props for now; future phase may add an onNewConnection callback.
</script>

<div class="empty-state">
  <!-- Illustration placeholder (FR-006) -->
  <div class="illustration-placeholder" aria-label="No connections illustration" data-testid="illustration" role="img">
    <svg
      class="illustration-svg"
      width="120"
      height="120"
      viewBox="0 0 120 120"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <!-- Simplified bowtie silhouette placeholder -->
      <circle cx="60" cy="60" r="56" stroke="currentColor" stroke-width="2" opacity="0.15" />
      <!-- Left node -->
      <rect x="12" y="46" width="28" height="28" rx="5" fill="currentColor" opacity="0.25" />
      <!-- Right node -->
      <rect x="80" y="46" width="28" height="28" rx="5" fill="currentColor" opacity="0.25" />
      <!-- Arrow broken in middle -->
      <line x1="40" y1="60" x2="52" y2="60" stroke="currentColor" stroke-width="2" stroke-linecap="round" opacity="0.4" />
      <line x1="68" y1="60" x2="80" y2="60" stroke="currentColor" stroke-width="2" stroke-linecap="round" opacity="0.4" />
      <!-- X in centre -->
      <line x1="56" y1="55" x2="64" y2="65" stroke="currentColor" stroke-width="2" stroke-linecap="round" opacity="0.5" />
      <line x1="64" y1="55" x2="56" y2="65" stroke="currentColor" stroke-width="2" stroke-linecap="round" opacity="0.5" />
    </svg>
  </div>

  <!-- Message (FR-006) -->
  <h3 class="empty-heading">No connections yet</h3>
  <p class="empty-body">
    No connections yet — click <strong>+ New Connection</strong> to link a producer to a consumer.
  </p>

  <!-- CTA — disabled in this phase (FR-012) -->
  <button
    class="cta-btn"
    disabled
    aria-disabled="true"
    title="Creating new connections is not yet available in this version"
  >
    + New Connection
  </button>
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 64px 24px;
    text-align: center;
    gap: 16px;
  }

  .illustration-placeholder {
    color: #605e5c;
    margin-bottom: 8px;
  }

  .illustration-svg {
    display: block;
  }

  .empty-heading {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 600;
    color: #242424;
  }

  .empty-body {
    margin: 0;
    font-size: 0.9rem;
    color: #605e5c;
    max-width: 360px;
    line-height: 1.5;
  }

  .cta-btn {
    margin-top: 8px;
    padding: 8px 20px;
    border-radius: 6px;
    border: 1px solid #d1d5db;
    background: #ffffff;
    color: #605e5c;
    font-size: 0.9rem;
    font-weight: 500;
    cursor: not-allowed;
    opacity: 0.45;
    pointer-events: none;
  }
</style>
