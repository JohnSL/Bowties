<script lang="ts">
  import MillerColumnsNav from '$lib/components/MillerColumns/MillerColumnsNav.svelte';

  // Page metadata
  const pageTitle = 'Configuration Navigator';
  const pageDescription = 'Navigate node CDI structures and configure elements';
</script>

<svelte:head>
  <title>{pageTitle} - LCC Bowties</title>
</svelte:head>

<div class="config-page">
  <header class="page-header">
    <div class="breadcrumb">
      <a href="/" class="breadcrumb-link">Home</a>
      <span class="breadcrumb-separator">›</span>
      <span class="breadcrumb-current">Configuration</span>
    </div>
    <h1 class="page-title">{pageTitle}</h1>
    <p class="page-description">{pageDescription}</p>
  </header>

  <main class="page-content">
    <MillerColumnsNav />
  </main>
</div>

<style>
  .config-page {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100%;
    overflow: hidden;
  }

  .page-header {
    padding: 20px 24px;
    background-color: var(--header-bg, #fff);
    border-bottom: 1px solid var(--border-color, #ddd);
  }

  .breadcrumb {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    margin-bottom: 12px;
    color: var(--text-secondary, #666);
  }

  .breadcrumb-link {
    color: var(--primary-color, #1976d2);
    text-decoration: none;
    transition: color 0.15s;
  }

  .breadcrumb-link:hover {
    color: var(--primary-dark, #1565c0);
    text-decoration: underline;
  }

  .breadcrumb-separator {
    color: var(--text-tertiary, #999);
  }

  .breadcrumb-current {
    font-weight: 500;
  }

  .page-title {
    margin: 0 0 8px 0;
    font-size: 24px;
    font-weight: 600;
    color: var(--text-primary, #333);
  }

  .page-description {
    margin: 0;
    font-size: 14px;
    color: var(--text-secondary, #666);
  }

  .page-content {
    flex: 1;
    overflow: hidden;
  }
</style>
