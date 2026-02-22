<script lang="ts">
import { onMount, onDestroy } from 'svelte';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { TrafficMessage, TrafficMonitorViewMode } from '$lib/api/types';

// Extend TrafficMessage with a frontend-only sequence number for stable keying
interface DisplayMessage extends TrafficMessage {
	seq: number;
}

interface AdvancedFrameRow {
	kind: 'frame';
	key: string;
	msg: DisplayMessage;
}

interface AdvancedSummaryRow {
	kind: 'summary';
	key: string;
	intent: string;
	byteCount: number;
	fields: Array<{ label: string; value: string }>;
}

type AdvancedRow = AdvancedFrameRow | AdvancedSummaryRow;

interface Props {
	isConnected: boolean;
	standalone?: boolean;
}

let { isConnected = false, standalone = false }: Props = $props();

// Monotonically incrementing counter — never reset, guarantees unique keys
let nextId = 0;

// State
let messages = $state<DisplayMessage[]>([]);
let isPaused = $state(false);
let showRawData = $state(false);
let autoScroll = $state(true);
let viewMode = $state<TrafficMonitorViewMode>('simple');

// Refs
let scrollContainer = $state<HTMLDivElement>();
let unlisten: UnlistenFn | null = null;

const MAX_MESSAGES = 500;

const UI = {
	colors: {
		sentRowBg: 'rgba(22, 163, 74, 0.08)',
		receivedRowBg: 'rgba(37, 99, 235, 0.08)',
		summaryRowBg: 'rgba(107, 114, 128, 0.08)',
		directionSent: '#16A34A',
		directionReceived: '#2563EB',
		directionUnknown: '#6B7280',
	},
} as const;



// Listen to LCC message events
onMount(async () => {
	console.log('[TrafficMonitor] Setting up event listener for lcc-message-received');
	unlisten = await listen<TrafficMessage>('lcc-message-received', (event) => {
		console.log('[TrafficMonitor] Received message:', event.payload);
		if (!isPaused) {
			addMessage(event.payload);
		} else {
			console.log('[TrafficMonitor] Message ignored (paused)');
		}
	});
	console.log('[TrafficMonitor] Event listener registered');
});

onDestroy(() => {
	if (unlisten) {
		unlisten();
	}
});

function addMessage(msg: TrafficMessage) {
	// Circular buffer: keep only last MAX_MESSAGES
	if (messages.length >= MAX_MESSAGES) {
		messages = messages.slice(-(MAX_MESSAGES - 1));
	}
	messages = [...messages, { ...msg, seq: nextId++ }];
	
	// Auto-scroll if enabled
	if (autoScroll && scrollContainer) {
		setTimeout(() => {
			scrollContainer.scrollTop = scrollContainer.scrollHeight;
		}, 0);
	}
}

function togglePause() {
	isPaused = !isPaused;
}

function clearMessages() {
	messages = [];
}

function toggleRawData() {
	showRawData = !showRawData;
}

function setViewMode(mode: TrafficMonitorViewMode) {
	viewMode = mode;
}

function toggleAutoScroll() {
	autoScroll = !autoScroll;
}

function frameRowStyle(direction: string | null): string {
	if (direction === 'S') return `font-weight: 700; background: ${UI.colors.sentRowBg};`;
	if (direction === 'R') return `background: ${UI.colors.receivedRowBg};`;
	return '';
}

function directionStyle(direction: string | null): string {
	if (direction === 'S') return `color: ${UI.colors.directionSent}; font-weight: 700;`;
	if (direction === 'R') return `color: ${UI.colors.directionReceived}; font-weight: 700;`;
	return `color: ${UI.colors.directionUnknown}; font-weight: 700;`;
}

function summaryRowStyle(): string {
	return `background: ${UI.colors.summaryRowBg};`;
}

// Format timestamp to HH:MM:SS.mmm
function formatTimestamp(timestamp: string): string {
	// Backend already sends in HH:MM:SS.mmm format, just return it
	return timestamp;
}

// Format alias as hex
function formatAlias(alias: number | null): string {
	if (alias === null || alias === undefined) return '---';
	return `0x${alias.toString(16).toUpperCase().padStart(3, '0')}`;
}

function isDatagramChunkMti(mti: string | null): boolean {
	return mti === 'DatagramOnly' || mti === 'DatagramFirst' || mti === 'DatagramMiddle' || mti === 'DatagramFinal';
}

function decodeAddress(bytes: number[]): number | null {
	if (bytes.length < 6) return null;
	return ((bytes[2] << 24) >>> 0) + (bytes[3] << 16) + (bytes[4] << 8) + bytes[5];
}

function toHexByte(value: number): string {
	return value.toString(16).toUpperCase().padStart(2, '0');
}

function toHexWord(value: number): string {
	return value.toString(16).toUpperCase().padStart(4, '0');
}

function toHexAddr(value: number): string {
	return value.toString(16).toUpperCase().padStart(8, '0');
}

function asciiPreview(bytes: number[], maxLen = 40): string {
	return bytes
		.slice(0, maxLen)
		.map((byte) => {
			if (byte === 0) return '\\0';
			if (byte >= 32 && byte <= 126) return String.fromCharCode(byte);
			return '.';
		})
		.join('');
}

function parseSnipStrings(payload: number[]): string[] {
	const values: string[] = [];
	let current: number[] = [];

	for (const byte of payload) {
		if (byte === 0) {
			if (current.length > 0) {
				values.push(String.fromCharCode(...current));
				current = [];
			}
			continue;
		}
		current.push(byte);
	}

	if (current.length > 0) {
		values.push(String.fromCharCode(...current));
	}

	return values;
}

function buildDatagramSummary(payload: number[], direction: string | null): { intent: string; byteCount: number; fields: Array<{ label: string; value: string }>; rawBytesHex: string } {
	const fields: Array<{ label: string; value: string }> = [];
	if (payload.length < 2) {
		return {
			intent: direction === 'S' ? 'Send datagram payload' : 'Receive datagram payload',
			byteCount: payload.length,
			fields: [{ label: 'Decode', value: 'Too short to decode protocol fields' }],
			rawBytesHex: payload.map((byte) => toHexByte(byte)).join(' '),
		};
	}

	const cmd = payload[0];
	const reply = payload[1];
	const address = decodeAddress(payload);
	const replyMeaning =
		reply === 0x50
			? 'Read Reply, success'
			: reply === 0x58
				? 'Read Reply, failure'
				: 'Other reply code';

	let intent = direction === 'S' ? 'Send datagram payload' : 'Receive datagram payload';
	if (cmd === 0x20 && reply === 0x40) {
		intent = direction === 'S'
			? `Request configuration read${address !== null ? ` at address 0x${toHexAddr(address)}` : ''}`
			: 'Received configuration read request';
	} else if (cmd === 0x20 && reply === 0x50) {
		intent = direction === 'S'
			? 'Sending configuration read response'
			: 'Received configuration read response';
	}

	fields.push({ label: 'Protocol', value: `0x${toHexByte(cmd)} = ${cmd === 0x20 ? 'Memory Configuration' : 'Unknown class'}` });

	fields.push({ label: 'Reply', value: `0x${toHexByte(reply)} = ${replyMeaning}` });

	if (address !== null) {
		fields.push({ label: 'Address', value: `0x${toHexAddr(address)} = memory location ${address}` });
	}

	let valueStart = 6;
	if (payload.length > 6 && (payload[6] === 0xfd || payload[6] === 0xfe || payload[6] === 0xff)) {
		const space = payload[6];
		const meaning = space === 0xfd ? 'Configuration space' : space === 0xfe ? 'All memory space' : 'CDI space';
		fields.push({ label: 'Space', value: `0x${toHexByte(space)} = ${meaning} (present in payload)` });
		valueStart = 7;
	} else {
		fields.push({ label: 'Space', value: 'Inferred from request context (not explicit in generic reply)' });
	}

	const valueBytes = payload.slice(valueStart);
	if (valueBytes.length > 0) {
		fields.push({ label: 'Value first byte', value: `0x${toHexByte(valueBytes[0])}${valueBytes[0] >= 32 && valueBytes[0] <= 126 ? ` ('${String.fromCharCode(valueBytes[0])}')` : ''}` });
		fields.push({ label: 'Text preview', value: `"${asciiPreview(valueBytes)}"` });
	}

	fields.push({ label: 'Direction', value: direction === 'S' ? 'Sent by this app' : direction === 'R' ? 'Received from node/network' : 'Unknown' });

	return { intent, byteCount: payload.length, fields, rawBytesHex: payload.map((byte) => toHexByte(byte)).join(' ') };
}

function buildSnipSummary(payload: number[], direction: string | null): { intent: string; byteCount: number; fields: Array<{ label: string; value: string }>; rawBytesHex: string } {
	const fields: Array<{ label: string; value: string }> = [];
	if (payload.length === 0) {
		return {
			intent: direction === 'S' ? 'Sending SNIP identity payload' : 'Receiving SNIP identity payload',
			byteCount: 0,
			fields: [{ label: 'Decode', value: 'No SNIP payload bytes assembled' }],
			rawBytesHex: '',
		};
	}

	const version = payload[0];
	const strings = parseSnipStrings(payload.slice(1));
	const labels = ['Manufacturer', 'Model', 'Hardware', 'Software', 'User Name', 'Description'];
	fields.push({ label: 'Version', value: `${version}` });

	for (let index = 0; index < labels.length; index++) {
		if (strings[index]) {
			fields.push({ label: labels[index], value: strings[index] });
		}
	}

	return {
		intent: direction === 'S' ? 'Sending node identity information (SNIP)' : 'Received node identity information (SNIP)',
		byteCount: payload.length,
		fields,
		rawBytesHex: payload.map((byte) => toHexByte(byte)).join(' '),
	};
}

function buildAdvancedRows(input: DisplayMessage[]): AdvancedRow[] {
	const rows: AdvancedRow[] = [];
	const datagramAssemblies = new Map<string, number[]>();
	const snipAssemblies = new Map<string, number[]>();

	for (const msg of input) {
		rows.push({ kind: 'frame', key: `frame-${msg.seq}`, msg });

		const mti = msg.mti;
		const bytes = msg.dataBytes ?? [];
		const flowKey = `${msg.sourceAlias ?? -1}->${msg.destAlias ?? -1}`;

		if (isDatagramChunkMti(mti) && bytes.length > 0) {
			if (mti === 'DatagramOnly' || mti === 'DatagramFirst') {
				datagramAssemblies.set(flowKey, [...bytes]);
			} else {
				const existing = datagramAssemblies.get(flowKey) ?? [];
				datagramAssemblies.set(flowKey, [...existing, ...bytes]);
			}

			if (mti === 'DatagramOnly' || mti === 'DatagramFinal') {
				const payload = datagramAssemblies.get(flowKey) ?? [];
				const summary = buildDatagramSummary(payload, msg.direction);
				rows.push({
					kind: 'summary',
					key: `summary-datagram-${msg.seq}`,
					intent: summary.intent,
					byteCount: summary.byteCount,
					fields: summary.fields,
				});
				datagramAssemblies.delete(flowKey);
			}
		}

		if (mti === 'SNIPResponse' && bytes.length >= 2) {
			const frameType = bytes[0];
			const payloadChunk = bytes.slice(2);

			if (frameType === 0x1a) {
				snipAssemblies.set(flowKey, [...payloadChunk]);
			} else {
				const existing = snipAssemblies.get(flowKey) ?? [];
				snipAssemblies.set(flowKey, [...existing, ...payloadChunk]);
			}

			if (frameType === 0x2a) {
				const payload = snipAssemblies.get(flowKey) ?? [];
				const summary = buildSnipSummary(payload, msg.direction);
				rows.push({
					kind: 'summary',
					key: `summary-snip-${msg.seq}`,
					intent: summary.intent,
					byteCount: summary.byteCount,
					fields: summary.fields,
				});
				snipAssemblies.delete(flowKey);
			}
		}
	}

	return rows;
}
</script>

<div class="traffic-monitor overflow-hidden {standalone ? 'h-full flex flex-col' : 'border border-gray-300 dark:border-gray-700 rounded-lg'}">
	<!-- Header -->
	<div class="header bg-gray-100 dark:bg-gray-800 px-4 py-2 flex items-center justify-between border-b border-gray-300 dark:border-gray-700">
		<div class="flex items-center gap-2">
			<span class="text-sm font-medium">
				Traffic Monitor
			</span>
			<span class="text-xs text-gray-500 dark:text-gray-400">
				({messages.length} messages)
			</span>
		</div>
		
		<div class="flex items-center gap-2">
			<div class="inline-flex rounded overflow-hidden border border-gray-300 dark:border-gray-600">
				<button
					onclick={() => setViewMode('simple')}
					class="px-3 py-1 text-xs transition-colors {viewMode === 'simple' ? 'bg-blue-600 text-white' : 'bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600'}"
				>
					Simple
				</button>
				<button
					onclick={() => setViewMode('advanced')}
					class="px-3 py-1 text-xs transition-colors {viewMode === 'advanced' ? 'bg-blue-600 text-white' : 'bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600'}"
				>
					Advanced
				</button>
			</div>

			<button
				onclick={togglePause}
				class="px-3 py-1 text-xs rounded {isPaused ? 'bg-amber-500 text-white' : 'bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600'} transition-colors"
				disabled={!isConnected}
			>
				{isPaused ? '▶ Resume' : '⏸ Pause'}
			</button>
			
			<button
				onclick={clearMessages}
				class="px-3 py-1 text-xs rounded bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
				disabled={!isConnected}
			>
				🗑 Clear
			</button>
			
			{#if viewMode === 'advanced'}
				<button
					onclick={toggleRawData}
					class="px-3 py-1 text-xs rounded {showRawData ? 'bg-blue-600 text-white' : 'bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600'} transition-colors"
				>
					{showRawData ? '📋 Raw' : '📄 Parsed'}
				</button>
			{/if}
			
			<button
				onclick={toggleAutoScroll}
				class="px-3 py-1 text-xs rounded {autoScroll ? 'bg-blue-600 text-white' : 'bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600'} transition-colors"
			>
				{autoScroll ? '📜 Auto' : '📜 Manual'}
			</button>
		</div>
	</div>
	
	<!-- Message list -->
	<div
		bind:this={scrollContainer}
		class="message-list overflow-y-auto bg-white dark:bg-gray-900 {standalone ? 'flex-1' : 'max-h-96'}"
	>
		{#if messages.length === 0}
			<div class="p-4 text-center text-gray-500 dark:text-gray-400">
				{#if !isConnected}
					Not connected to LCC network
				{:else}
					No messages yet. Waiting for traffic...
				{/if}
			</div>
		{:else}
			{#if viewMode === 'simple'}
				{#each messages as msg (msg.seq)}
					<div class="message-row font-mono text-xs px-3 py-0.5 border-b border-gray-100 dark:border-gray-800" style={frameRowStyle(msg.direction)}>
						<div class="flex items-start gap-2">
							<span class="text-gray-400 dark:text-gray-600 w-24 shrink-0">
								{formatTimestamp(msg.timestamp)}
							</span>
							<span class="w-4 shrink-0" style={directionStyle(msg.direction)}>
								{msg.direction || '?'}:
							</span>
							<span class="text-gray-800 dark:text-gray-200 flex-1 break-all">
								{msg.decodedPayload || '(no data)'}
							</span>
						</div>
					</div>
				{/each}
			{:else}
				{#each buildAdvancedRows(messages) as row (row.key)}
					{#if row.kind === 'summary'}
						<div class="summary-row px-3 py-1.5 border-b border-gray-100 dark:border-gray-800" style={summaryRowStyle()}>
							<div class="summary-content text-xs text-gray-700 dark:text-gray-300">
								<div class="font-semibold mb-1">{row.intent}: {row.byteCount} bytes</div>
								<table class="summary-table">
									<tbody>
										{#each row.fields as field}
											<tr>
												<td class="summary-label">{field.label}</td>
												<td class="summary-value">{field.value}</td>
											</tr>
										{/each}
									</tbody>
								</table>
							</div>
						</div>
					{:else}
						<div class="message-row font-mono text-xs px-3 py-0.5 border-b border-gray-100 dark:border-gray-800" style={frameRowStyle(row.msg.direction)}>
						<div class="flex items-start gap-2">
							<!-- Timestamp -->
							<span class="text-gray-400 dark:text-gray-600 w-24 shrink-0">
								{formatTimestamp(row.msg.timestamp)}
							</span>

							<!-- Direction -->
							<span class="w-4 shrink-0" style={directionStyle(row.msg.direction)}>
								{row.msg.direction || '?'}:
							</span>

							<!-- Source → Dest -->
							<span class="text-gray-600 dark:text-gray-400 w-28 shrink-0">
								{formatAlias(row.msg.sourceAlias)}
								{#if row.msg.destAlias !== null && row.msg.destAlias !== undefined}
									→ {formatAlias(row.msg.destAlias)}
								{/if}
							</span>

							<!-- MTI -->
								<span class="text-purple-600 dark:text-purple-400 w-36 shrink-0 truncate" title={row.msg.mtiLabel || row.msg.mti || 'Unknown'}>
								{row.msg.mtiLabel || row.msg.mti || 'Unknown'}
							</span>

							<!-- Technical payload or raw frame -->
							<span class="text-gray-800 dark:text-gray-200 flex-1 whitespace-pre-wrap break-words">
								{#if showRawData}
									{row.msg.frame}
								{:else}
									{row.msg.technicalDetails || row.msg.decodedPayload || '(no data)'}
									{#if row.msg.nodeId}
										<span class="text-blue-600 dark:text-blue-400 ml-2">
											[{row.msg.nodeId}]
										</span>
									{/if}
								{/if}
							</span>
						</div>
						</div>
					{/if}
				{/each}
			{/if}
		{/if}
	</div>
</div>

<style>
	.message-list {
		scrollbar-width: thin;
	}
	
	.message-list::-webkit-scrollbar {
		width: 8px;
	}
	
	.message-list::-webkit-scrollbar-track {
		background: transparent;
	}
	
	.message-list::-webkit-scrollbar-thumb {
		background: #888;
		border-radius: 4px;
	}
	
	.message-list::-webkit-scrollbar-thumb:hover {
		background: #555;
	}

	.summary-table {
		border-collapse: collapse;
		margin-top: 0.25rem;
		margin-bottom: 0.25rem;
		min-width: 10rem;
	}

	.summary-table td {
		padding: 0.1rem 0.4rem;
		vertical-align: top;
	}

	.summary-table tr:nth-child(odd) {
		background: rgba(107, 114, 128, 0.08);
	}

	.summary-table tr:nth-child(even) {
		background: rgba(107, 114, 128, 0.14);
	}

	.summary-label {
		font-weight: 600;
		padding-left: 0.5rem;
		padding-right: 1.25rem;
		white-space: nowrap;
		color: #4b5563;
		min-width: 1.5rem;
	}

	.summary-value {
		color: #1f2937;
		padding-left: 0.35rem;
	}

	.summary-content {
		margin-left: 0.5rem;
	}
</style>
