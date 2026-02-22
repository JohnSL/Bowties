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
	class="btn-secondary flex items-center gap-2 !rounded"
	class:opacity-75={isRefreshing}
	onclick={handleRefresh}
	{disabled}
	aria-label="Refresh node status"
	title="Refresh node status"
>
	<span class="{isRefreshing ? 'animate-spin inline-block' : ''}">⟳</span>
	<span>
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
