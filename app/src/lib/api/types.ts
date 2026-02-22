/**
 * API Type Definitions for Configuration Value Reading
 * 
 * TypeScript interfaces for config value reading operations,
 * matching Rust backend types from commands/cdi.rs
 */

// T014: ConfigValue type (discriminated union)
export type ConfigValue = 
    | { type: 'Int'; value: number; size_bytes: number }
    | { type: 'String'; value: string; size_bytes: number }
    | { type: 'EventId'; value: number[] }  // Array of 8 bytes
    | { type: 'Float'; value: number }
    | { type: 'Invalid'; error: string };

// T015: ConfigValueWithMetadata interface
export interface ConfigValueWithMetadata {
    value: ConfigValue;
    memory_address: number;
    address_space: number;
    element_path: string[];
    timestamp: string;
}

// T017: ProgressStatus type (discriminated union)
export type ProgressStatus = 
    | { type: 'Starting' }
    | { type: 'ReadingNode'; node_name: string }
    | { type: 'NodeComplete'; node_name: string; success: boolean }
    | { type: 'Cancelled' }
    | { type: 'Complete'; success_count: number; fail_count: number };

// T016: ReadProgressState interface
export interface ReadProgressState {
    totalNodes: number;
    currentNodeIndex: number;
    currentNodeName: string;
    currentNodeId: string;
    totalElements: number;
    elementsRead: number;
    elementsFailed: number;
    percentage: number;  // 0-100
    status: ProgressStatus;
}

// T018: ReadAllConfigValuesResponse interface
export interface ReadAllConfigValuesResponse {
    nodeId: string;
    values: Record<string, ConfigValueWithMetadata>;
    totalElements: number;
    successfulReads: number;
    failedReads: number;
    durationMs: number;
}

// T019: ConfigValueMap type
export type ConfigValueMap = Map<string, ConfigValueWithMetadata>;

/**
 * Helper to generate cache key for config values
 * Format: "nodeId:elementPath"
 */
export function getCacheKey(nodeId: string, elementPath: string[]): string {
    return `${nodeId}:${elementPath.join('/')}`;
}

/**
 * Traffic Monitor Types
 */

// Traffic message matching MessageReceivedEvent from Rust backend
export interface TrafficMessage {
    frame: string;
    header: number | null;
    dataBytes: number[] | null;
    mti: string | null;
    mtiLabel: string | null;
    sourceAlias: number | null;
    timestamp: string;
    direction: string | null;  // "S" for sent, "R" for received
    decodedPayload: string | null;
    technicalDetails: string | null;
    nodeId: string | null;
    destAlias: number | null;
}

// Display and formatting options for traffic monitor
export interface FormatOptions {
    showRawData: boolean;
    showTimestamps: boolean;
    maxMessages: number;
}

export type TrafficMonitorViewMode = 'simple' | 'advanced';
