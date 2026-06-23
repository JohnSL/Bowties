<script lang="ts">
  /**
   * AboutDialog — Modal showing application name, version, copyright, and links.
   *
   * Keyboard: Escape or Enter → close.
   */
  import { onMount, onDestroy } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  let version = $state('');
  let closeBtn: HTMLButtonElement | undefined = $state();

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' || event.key === 'Enter') {
      event.preventDefault();
      onClose();
    }
  }

  onMount(async () => {
    closeBtn?.focus();
    window.addEventListener('keydown', handleKeydown);
    try {
      version = await getVersion();
    } catch {
      version = 'unknown';
    }
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
  });
</script>

<div class="about-overlay" role="presentation">
  <div
    id="about-dialog"
    class="about-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="about-title"
  >
    <h2 id="about-title" class="about-name">Bowties</h2>
    <p class="about-version">Version {version}</p>
    <p class="about-description">An LCC/OpenLCB node configuration tool</p>
    <p class="about-copyright">Copyright © 2026 John Socha-Leialoha</p>
    <p class="about-license">Licensed under MIT or Apache-2.0</p>
    <p class="about-link">
      <a href="https://github.com/JohnSL/Bowties" target="_blank" rel="noopener noreferrer">
        github.com/JohnSL/Bowties
      </a>
    </p>

    <div class="about-actions">
      <button
        class="about-btn"
        bind:this={closeBtn}
        onclick={onClose}
      >
        Close
      </button>
    </div>
  </div>
</div>

<style>
  .about-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }

  .about-dialog {
    background: white;
    border-radius: 8px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
    max-width: 400px;
    min-width: 280px;
    padding: 32px;
    text-align: center;
    animation: slideIn 0.2s ease-out;
  }

  @keyframes slideIn {
    from {
      transform: translateY(-20px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  .about-name {
    margin: 0 0 4px 0;
    font-size: 22px;
    font-weight: 700;
    color: #1f2937;
  }

  .about-version {
    margin: 0 0 16px 0;
    font-size: 14px;
    color: #6b7280;
  }

  .about-description {
    margin: 0 0 12px 0;
    font-size: 14px;
    color: #374151;
  }

  .about-copyright {
    margin: 0 0 4px 0;
    font-size: 13px;
    color: #6b7280;
  }

  .about-license {
    margin: 0 0 12px 0;
    font-size: 13px;
    color: #6b7280;
  }

  .about-link {
    margin: 0 0 24px 0;
    font-size: 13px;
  }

  .about-link a {
    color: #0066cc;
    text-decoration: none;
  }

  .about-link a:hover {
    text-decoration: underline;
  }

  .about-actions {
    display: flex;
    justify-content: center;
  }

  .about-btn {
    padding: 8px 24px;
    border: none;
    border-radius: 4px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    background: #0066cc;
    color: white;
    transition: background 0.2s ease;
  }

  .about-btn:hover {
    background: #0052a3;
  }

  .about-btn:active {
    background: #003d7a;
  }
</style>
