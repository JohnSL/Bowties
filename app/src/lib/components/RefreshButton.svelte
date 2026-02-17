<script lang="ts">
	import { refreshAllNodes } from '$lib/api/tauri';

	// Props
	interface Props {
		onRefreshComplete?: (nodes: any[]) => void;
		disabled?: boolean;
	}

	let { onRefreshComplete, disabled = false }: Props = $props();

	// State
	let isRefreshing = $state(false);
	let error = $state<string | null>(null);

	async function handleRefresh() {
		if (isRefreshing || disabled) return;

		isRefreshing = true;
		error = null;

		try {
			const nodes = await refreshAllNodes(500); // 500ms timeout per node
			onRefreshComplete?.(nodes);
		} catch (e) {
			console.error('Failed to refresh nodes:', e);
			error = e instanceof Error ? e.message : 'Unknown error occurred';
		} finally {
			isRefreshing = false;
		}
	}
</script>

<button
	type="button"
	class="refresh-button"
	class:refreshing={isRefreshing}
	onclick={handleRefresh}
	{disabled}
	aria-label="Refresh node status"
	title="Refresh node status"
>
	<span class="icon" class:spinning={isRefreshing}>⟳</span>
	<span class="text">
		{#if isRefreshing}
			Refreshing...
		{:else}
			Refresh
		{/if}
	</span>
</button>

{#if error}
	<div class="error-message" role="alert">
		{error}
	</div>
{/if}

<style>
	.refresh-button {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 1rem;
		border: 1px solid #ccc;
		border-radius: 4px;
		background: white;
		font-size: 0.875rem;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.refresh-button:hover:not(:disabled) {
		background: #f5f5f5;
		border-color: #999;
	}

	.refresh-button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.refresh-button.refreshing {
		background: #f0f9ff;
		border-color: #0284c7;
	}

	.icon {
		font-size: 1.25rem;
		line-height: 1;
		transition: transform 0.3s ease;
	}

	.icon.spinning {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}

	.error-message {
		margin-top: 0.5rem;
		padding: 0.5rem;
		background: #fef2f2;
		border: 1px solid #ef4444;
		border-radius: 4px;
		color: #991b1b;
		font-size: 0.875rem;
	}
</style>
