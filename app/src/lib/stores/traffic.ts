/**
 * Traffic Monitor Store
 * 
 * Shared state for traffic monitor across components, enabling future
 * extraction to a dedicated window while maintaining shared data.
 */

import { writable, derived } from 'svelte/store';
import type { TrafficMessage } from '$lib/api/types';

interface TrafficState {
	messages: TrafficMessage[];
	isPaused: boolean;
	maxMessages: number;
}

const initialState: TrafficState = {
	messages: [],
	isPaused: false,
	maxMessages: 500,
};

function createTrafficStore() {
	const { subscribe, set, update } = writable<TrafficState>(initialState);

	return {
		subscribe,
		
		// Add a message to the buffer
		addMessage: (message: TrafficMessage) => {
			update(state => {
				if (state.isPaused) {
					return state;
				}
				
				let newMessages = [...state.messages, message];
				
				// Circular buffer: keep only last maxMessages
				if (newMessages.length > state.maxMessages) {
					newMessages = newMessages.slice(-(state.maxMessages - 1));
				}
				
				return {
					...state,
					messages: newMessages,
				};
			});
		},
		
		// Clear all messages
		clearMessages: () => {
			update(state => ({
				...state,
				messages: [],
			}));
		},
		
		// Toggle pause state
		togglePause: () => {
			update(state => ({
				...state,
				isPaused: !state.isPaused,
			}));
		},
		
		// Set pause state explicitly
		setPaused: (paused: boolean) => {
			update(state => ({
				...state,
				isPaused: paused,
			}));
		},
		
		// Set max messages limit
		setMaxMessages: (max: number) => {
			update(state => ({
				...state,
				maxMessages: max,
			}));
		},
		
		// Reset to initial state
		reset: () => {
			set(initialState);
		},
	};
}

export const trafficStore = createTrafficStore();

// Derived stores for convenient access
export const messages = derived(trafficStore, $state => $state.messages);
export const isPaused = derived(trafficStore, $state => $state.isPaused);
export const messageCount = derived(trafficStore, $state => $state.messages.length);
