<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { createEventDispatcher, onMount } from 'svelte';
  import { getLayoutConnections, saveLayoutConnections } from '$lib/api/layout';
  import { layoutStore } from '$lib/stores/layout.svelte';

  type AdapterType = 'tcp' | 'gridConnectSerial' | 'mergGridConnectSerial' | 'slcanSerial';
  type FlowControl = 'none' | 'rtsCts' | 'xonXoff';

  /** Known device presets with auto-filled serial parameters. */
  type DevicePreset = 'tcp' | 'rrcirkits' | 'sprog-usblcc' | 'sprog-pilcc' | 'merg-canrs' | 'slcan' | 'otherGc' | 'otherSlcan';

  /**
   * Advanced settings visibility:
   * - 'none': no baud/flow override (fixed preset — SPROG, SLCAN)
   * - 'toggle': hidden by default, revealed by "Show additional settings" checkbox
   * - 'always': always visible (for "Other" presets where user must configure)
   */
  type AdvancedMode = 'none' | 'toggle' | 'always';

  interface DeviceInfo {
    label: string;
    hint: string;
    adapterType: AdapterType;
    defaultBaud: number;
    /** Valid baud rate options for the dropdown. */
    baudOptions: number[];
    flowControl: FlowControl;
    /** Whether the user can change flow control (only "Other" presets). */
    flowControlOverridable: boolean;
    /** Controls visibility of baud/flow fields. */
    advancedMode: AdvancedMode;
  }

  /** Standard baud rates supported by GridConnect adapters (matches JMRI). */
  const GC_BAUD_OPTIONS = [57600, 115200, 230400, 250000, 333333, 460800];
  /** Standard baud rates for SLCAN adapters. */
  const SLCAN_BAUD_OPTIONS = [57600, 115200, 230400, 250000, 460800, 921600];

  const DEVICE_PRESETS: Record<DevicePreset, DeviceInfo> = {
    tcp: {
      label: 'Network hub (TCP)',
      hint: 'JMRI, WifiTrax, or standalone TCP/IP bridge',
      adapterType: 'tcp',
      defaultBaud: 0,
      baudOptions: [],
      flowControl: 'none',
      flowControlOverridable: false,
      advancedMode: 'none',
    },
    rrcirkits: {
      label: 'RR-CirKits LCC Buffer-USB',
      hint: 'Also compatible with RR-CirKits LCC to Loconet Bridge',
      adapterType: 'gridConnectSerial',
      defaultBaud: 57600,
      baudOptions: GC_BAUD_OPTIONS,
      flowControl: 'none',
      flowControlOverridable: false,
      advancedMode: 'toggle',
    },
    'sprog-usblcc': {
      label: 'SPROG USB-LCC',
      hint: 'SPROG DCC Ltd USB-LCC CAN adapter',
      adapterType: 'gridConnectSerial',
      defaultBaud: 460800,
      baudOptions: [460800],
      flowControl: 'rtsCts',
      flowControlOverridable: false,
      advancedMode: 'none',
    },
    'sprog-pilcc': {
      label: 'SPROG PI-LCC',
      hint: 'SPROG DCC Ltd Raspberry Pi LCC hat',
      adapterType: 'gridConnectSerial',
      defaultBaud: 460800,
      baudOptions: [460800],
      flowControl: 'rtsCts',
      flowControlOverridable: false,
      advancedMode: 'none',
    },
    'merg-canrs': {
      label: 'MERG CAN-RS / CANUSB4',
      hint: 'MERG CAN-RS, CANUSB4, or compatible adapter',
      adapterType: 'mergGridConnectSerial',
      defaultBaud: 57600,
      baudOptions: GC_BAUD_OPTIONS,
      flowControl: 'none',
      flowControlOverridable: false,
      advancedMode: 'toggle',
    },
    slcan: {
      label: 'Canable / Lawicell CANUSB',
      hint: 'SLCAN-compatible USB-CAN adapter',
      adapterType: 'slcanSerial',
      defaultBaud: 115200,
      baudOptions: [115200],
      flowControl: 'none',
      flowControlOverridable: false,
      advancedMode: 'none',
    },
    otherGc: {
      label: 'Other GridConnect adapter',
      hint: 'CAN2USBINO or other GridConnect device',
      adapterType: 'gridConnectSerial',
      defaultBaud: 57600,
      baudOptions: GC_BAUD_OPTIONS,
      flowControl: 'none',
      flowControlOverridable: true,
      advancedMode: 'always',
    },
    otherSlcan: {
      label: 'Other SLCAN adapter',
      hint: 'Any slcand-compatible adapter not listed above',
      adapterType: 'slcanSerial',
      defaultBaud: 115200,
      baudOptions: SLCAN_BAUD_OPTIONS,
      flowControl: 'none',
      flowControlOverridable: true,
      advancedMode: 'always',
    },
  };

  /** Ordered list for the dropdown. */
  const DEVICE_ORDER: DevicePreset[] = [
    'tcp', 'rrcirkits', 'sprog-usblcc', 'sprog-pilcc', 'merg-canrs', 'slcan', 'otherGc', 'otherSlcan',
  ];

  interface ConnectionConfig {
    id: string;
    name: string;
    adapterType: AdapterType;
    host?: string;
    port?: number;
    serialPort?: string;
    baudRate?: number;
    flowControl: FlowControl;
  }

  const dispatch = createEventDispatcher<{ connected: { config: ConnectionConfig } }>();

  // crypto.randomUUID() requires Safari 15.4+ / WebKit 615+ (macOS 12+, Ubuntu 22.04+).
  // Use getRandomValues() which is available everywhere Tauri runs (Safari 11+, WebKit 606+).
  function generateUUID(): string {
    const bytes = new Uint8Array(16);
    crypto.getRandomValues(bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 1
    return [...bytes].map((b, i) =>
      ([4, 6, 8, 10].includes(i) ? '-' : '') + b.toString(16).padStart(2, '0')
    ).join('');
  }

  // Saved connections loaded from the active layout's manifest (Spec 013 / S7).
  let savedConnections = $state<ConnectionConfig[]>([]);
  let sortedConnections = $derived([...savedConnections].sort((a, b) => a.name.localeCompare(b.name)));

  // Track the layout path we last loaded from so we can refresh when the
  // active layout changes (open/switch/close).
  let loadedForPath = $state<string | null>(null);
  let activeLayoutPath = $derived(layoutStore.activeContext?.rootPath ?? null);

  // Available serial ports
  let availablePorts = $state<string[]>([]);
  let portsLoading = $state(false);

  // Modal state
  let showModal = $state(false);
  let editingId = $state<string | null>(null); // null = new connection

  // Form fields (shared between add and edit)
  let formName = $state('');
  let formDevice = $state<DevicePreset>('tcp');
  let formHost = $state('localhost');
  let formTcpPort = $state(12021);
  let formSerialPort = $state('');
  let formBaudRate = $state(57600);
  let formFlowControl = $state<FlowControl>('none');
  let formShowAdditional = $state(false);

  // Connection in progress
  let connectingId = $state<string | null>(null);
  let errorMessage = $state('');

  // Pending delete confirmation
  let confirmDeleteId = $state<string | null>(null);

  // Derived: current device preset info
  let deviceInfo = $derived(DEVICE_PRESETS[formDevice]);
  let isSerial = $derived(deviceInfo.adapterType !== 'tcp');
  /** Whether baud/flow fields are currently visible. */
  let showBaudFields = $derived(
    deviceInfo.advancedMode === 'always' ||
    (deviceInfo.advancedMode === 'toggle' && formShowAdditional)
  );

  // Apply preset defaults when device changes (only when adding, not editing)
  $effect(() => {
    const preset = DEVICE_PRESETS[formDevice];
    if (editingId === null) {
      formBaudRate = preset.defaultBaud;
      formFlowControl = preset.flowControl;
      formShowAdditional = false;
    }
  });

  onMount(() => {
    void refreshPorts();
  });

  // Load connections whenever the active layout changes (including initial
  // mount once the layout becomes available, and when the user switches
  // layouts). Effect runs once on mount and again on every path change.
  $effect(() => {
    const path = activeLayoutPath;
    if (path === loadedForPath) return;
    loadedForPath = path;
    void loadConnections();
  });

  async function loadConnections() {
    const path = activeLayoutPath;
    if (!path) {
      savedConnections = [];
      return;
    }
    try {
      savedConnections = await getLayoutConnections(path);
    } catch (e) {
      console.error('Failed to load layout connections:', e);
      savedConnections = [];
    }
  }

  async function refreshPorts() {
    portsLoading = true;
    try {
      availablePorts = await invoke<string[]>('list_serial_ports');
      if (availablePorts.length > 0 && !formSerialPort) {
        formSerialPort = availablePorts[0];
      }
    } catch (e) {
      console.error('Failed to list serial ports:', e);
      availablePorts = [];
    } finally {
      portsLoading = false;
    }
  }

  async function persistConnections() {
    const path = activeLayoutPath;
    if (!path) {
      // No active layout — nothing to persist against. This should not
      // happen in normal use because the layout picker (S6) gates the app
      // until a layout is open.
      console.warn('Refusing to save connections: no active layout');
      return;
    }
    try {
      await saveLayoutConnections(path, savedConnections);
    } catch (e) {
      console.error('Failed to save layout connections:', e);
      errorMessage = `Failed to save connections: ${e}`;
    }
  }

  function openAddModal() {
    editingId = null;
    formName = '';
    formDevice = 'tcp';
    formHost = 'localhost';
    formTcpPort = 12021;
    formSerialPort = availablePorts[0] ?? '';
    formBaudRate = DEVICE_PRESETS['tcp'].defaultBaud;
    formFlowControl = 'none';
    formShowAdditional = false;
    errorMessage = '';
    showModal = true;
  }

  /** Infer the best device preset from a saved config's parameters. */
  function inferPreset(conn: ConnectionConfig): DevicePreset {
    if (conn.adapterType === 'tcp') return 'tcp';
    if (conn.adapterType === 'slcanSerial') {
      return conn.baudRate === 115200 && conn.flowControl === 'none' ? 'slcan' : 'otherSlcan';
    }
    if (conn.adapterType === 'mergGridConnectSerial') return 'merg-canrs';
    // GridConnect — match known presets by baud + flow control
    if (conn.flowControl === 'rtsCts' && conn.baudRate === 460800) {
      // Could be USB-LCC or PI-LCC; default to USB-LCC since it's more common
      return 'sprog-usblcc';
    }
    if (conn.baudRate === 57600 && conn.flowControl === 'none') return 'rrcirkits';
    return 'otherGc';
  }

  function openEditModal(conn: ConnectionConfig) {
    editingId = conn.id;
    formName = conn.name;
    formDevice = inferPreset(conn);
    formHost = conn.host ?? 'localhost';
    formTcpPort = conn.port ?? 12021;
    formSerialPort = conn.serialPort ?? (availablePorts[0] ?? '');
    const preset = DEVICE_PRESETS[inferPreset(conn)];
    formBaudRate = conn.baudRate ?? preset.defaultBaud;
    formFlowControl = conn.flowControl ?? 'none';
    // Expand additional settings if saved baud differs from default
    formShowAdditional = (conn.baudRate !== undefined && conn.baudRate !== preset.defaultBaud);
    errorMessage = '';
    showModal = true;
  }

  function closeModal() {
    showModal = false;
  }

  async function submitForm() {
    if (!formName.trim()) return;

    try {
      const preset = DEVICE_PRESETS[formDevice];
      const config: ConnectionConfig = {
        id: editingId ?? generateUUID(),
        name: formName.trim(),
        adapterType: preset.adapterType,
        flowControl: preset.flowControlOverridable ? formFlowControl : preset.flowControl,
      };

      if (preset.adapterType === 'tcp') {
        config.host = formHost;
        config.port = formTcpPort;
      } else {
        config.serialPort = formSerialPort;
        config.baudRate = formBaudRate;
      }

      if (editingId !== null) {
        savedConnections = savedConnections.map(c => c.id === editingId ? config : c);
      } else {
        savedConnections = [...savedConnections, config];
      }
      await persistConnections();
      closeModal();
    } catch (e) {
      errorMessage = `Failed to save connection: ${e}`;
    }
  }

  async function deleteConnection(id: string) {
    savedConnections = savedConnections.filter(c => c.id !== id);
    confirmDeleteId = null;
    await persistConnections();
  }

  async function connect(config: ConnectionConfig) {
    connectingId = config.id;
    errorMessage = '';
    try {
      await invoke('connect_lcc', { config });
      dispatch('connected', { config });
    } catch (e) {
      errorMessage = `Connection failed: ${e}`;
    } finally {
      connectingId = null;
    }
  }

  function adapterLabel(type: AdapterType): string {
    switch (type) {
      case 'tcp': return 'TCP';
      case 'gridConnectSerial': return 'Serial';
      case 'mergGridConnectSerial': return 'MERG';
      case 'slcanSerial': return 'SLCAN';
    }
  }

  function connectionSummary(c: ConnectionConfig): string {
    if (c.adapterType === 'tcp') return `${c.host ?? 'localhost'}:${c.port ?? 12021}`;
    return c.serialPort ?? '?';
  }
</script>

<div class="cm-card">
  <div class="cm-card-header">
    <h2>Connect to LCC Network</h2>
    <button class="cm-add-icon-btn" onclick={openAddModal} title="Add connection" aria-label="Add connection">+</button>
  </div>

  {#if errorMessage}
    <div class="cm-error" role="alert">{errorMessage}</div>
  {/if}

  <!-- ── Saved connections list ──────────────────────────── -->
  {#if sortedConnections.length > 0}
    <table class="cm-table">
      <tbody>
        {#each sortedConnections as conn (conn.id)}
          <tr class="cm-row">
            <td class="cm-col-connect">
              <button
                class="btn-primary cm-connect-btn"
                onclick={() => connect(conn)}
                disabled={connectingId !== null}
              >
                {connectingId === conn.id ? 'Connecting…' : 'Connect'}
              </button>
            </td>
            <td class="cm-col-name">
              <span class="cm-item-name" title={conn.name}>{conn.name}</span>
            </td>
            <td class="cm-col-type">
              <span class="cm-badge {conn.adapterType}">{adapterLabel(conn.adapterType)}</span>
            </td>
            <td class="cm-col-detail">
              <span class="cm-item-detail">{connectionSummary(conn)}</span>
            </td>
            <td class="cm-col-actions">
              {#if confirmDeleteId === conn.id}
                <span class="cm-delete-confirm">
                  <button
                    class="cm-delete-confirm-btn"
                    onclick={() => deleteConnection(conn.id)}
                  >Delete</button>
                  <button
                    class="btn-secondary cm-delete-cancel-btn"
                    onclick={() => confirmDeleteId = null}
                  >Cancel</button>
                </span>
              {:else}
                <button
                  class="cm-edit-btn"
                  onclick={() => openEditModal(conn)}
                  title="Edit {conn.name}"
                  aria-label="Edit {conn.name}"
                  disabled={connectingId !== null}
                >🖊</button>
                <button
                  class="cm-delete-btn"
                  onclick={() => confirmDeleteId = conn.id}
                  title="Remove this connection"
                  aria-label="Remove {conn.name}"
                  disabled={connectingId !== null}
                >×</button>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>

<!-- ── Modal dialog ──────────────────────────────────────── -->
{#if showModal}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="cm-overlay" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) closeModal(); }}>
    <dialog class="cm-dialog" open aria-modal="true" aria-label={editingId === null ? 'Add connection' : 'Edit connection'}>
      <div class="cm-dialog-header">
        <h3>{editingId === null ? 'Add connection' : 'Edit connection'}</h3>
        <button class="cm-close-btn" onclick={closeModal} aria-label="Close">×</button>
      </div>

      <form class="cm-form" onsubmit={(e) => { e.preventDefault(); submitForm(); }}>
        <!-- Name -->
        <label class="cm-field">
          <span>Name</span>
          <input type="text" bind:value={formName} placeholder="My layout hub" required />
        </label>

        <!-- Device picker -->
        <label class="cm-field">
          <span>Device</span>
          <select bind:value={formDevice}>
            {#each DEVICE_ORDER as key}
              <option value={key}>{DEVICE_PRESETS[key].label}</option>
            {/each}
          </select>
        </label>
        <p class="cm-hint cm-device-hint">{deviceInfo.hint}</p>

        <!-- TCP fields -->
        {#if !isSerial}
          <label class="cm-field">
            <span>Host</span>
            <input type="text" bind:value={formHost} placeholder="localhost" />
          </label>
          <label class="cm-field">
            <span>Port</span>
            <input type="number" bind:value={formTcpPort} min="1" max="65535" />
          </label>
        {/if}

        <!-- Serial fields -->
        {#if isSerial}
          <div class="cm-field cm-port-row">
            <div class="cm-field-inner">
              <span>COM port</span>
              {#if availablePorts.length > 0}
                <select bind:value={formSerialPort}>
                  {#each availablePorts as p}
                    <option value={p}>{p}</option>
                  {/each}
                </select>
              {:else}
                <input type="text" bind:value={formSerialPort} placeholder="COM3" />
              {/if}
            </div>
            <button
              type="button"
              class="btn-secondary cm-refresh-btn"
              onclick={refreshPorts}
              disabled={portsLoading}
              title="Refresh port list"
            >
              {portsLoading ? '…' : '⟳'}
            </button>
          </div>

          {#if deviceInfo.advancedMode === 'toggle'}
            <label class="cm-toggle-label">
              <input type="checkbox" bind:checked={formShowAdditional} />
              <span>Show additional connection settings</span>
            </label>
          {/if}

          {#if showBaudFields}
            <label class="cm-field">
              <span>Baud rate</span>
              <select bind:value={formBaudRate}>
                {#each deviceInfo.baudOptions as rate}
                  <option value={rate}>{rate.toLocaleString()} baud</option>
                {/each}
              </select>
            </label>
            {#if deviceInfo.flowControlOverridable}
              <label class="cm-field">
                <span>Flow control</span>
                <select bind:value={formFlowControl}>
                  <option value="none">None</option>
                  <option value="rtsCts">RTS/CTS</option>
                  <option value="xonXoff">XON/XOFF</option>
                </select>
              </label>
            {/if}
          {/if}
        {/if}

        <div class="cm-dialog-footer">
          <button type="button" class="btn-secondary" onclick={closeModal}>Cancel</button>
          <button type="submit" class="btn-primary" disabled={!formName.trim()}>
            {editingId === null ? 'Add' : 'Save'}
          </button>
        </div>
      </form>
    </dialog>
  </div>
{/if}

<style>
  .cm-card {
    background: white;
    border: 1px solid #e5e7eb;
    border-radius: 8px;
    padding: 2rem;
    min-width: 360px;
    width: max-content;
    max-width: min(560px, 95vw);
    box-shadow: 0 4px 16px rgba(0,0,0,0.08);
  }

  .cm-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1.25rem;
  }

  .cm-card-header h2 {
    margin: 0;
    color: #2563eb;
    font-size: 1.1rem;
  }

  .cm-add-icon-btn {
    background: none;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    cursor: pointer;
    font-size: 18px;
    line-height: 1;
    padding: 2px 8px;
    color: #2563eb;
    transition: background 0.15s, border-color 0.15s;
  }
  .cm-add-icon-btn:hover { background: #eff6ff; border-color: #2563eb; }

  .cm-error {
    background: #fef2f2;
    border: 1px solid #fca5a5;
    color: #b91c1c;
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    font-size: 13px;
    margin-bottom: 1rem;
  }

  /* ── Saved list ── */
  .cm-table {
    width: 100%;
    border-collapse: collapse;
    margin-bottom: 0.5rem;
    font-size: 13px;
  }

  .cm-row td {
    padding: 0.35rem 0.4rem;
    vertical-align: middle;
  }

  .cm-row:not(:last-child) td {
    border-bottom: 1px solid #f3f4f6;
  }

  .cm-col-connect { width: 1%; white-space: nowrap; padding-left: 0 !important; }
  .cm-col-actions { width: 1%; white-space: nowrap; padding-right: 0 !important; text-align: right; }

  .cm-item-name {
    font-weight: 600;
    color: #111827;
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 150px;
  }

  .cm-badge {
    font-size: 11px;
    font-weight: 600;
    border-radius: 4px;
    padding: 1px 5px;
    white-space: nowrap;
  }
  .cm-badge.tcp             { background: #dbeafe; color: #1d4ed8; }
  .cm-badge.gridConnectSerial { background: #dcfce7; color: #15803d; }
  .cm-badge.slcanSerial     { background: #fef3c7; color: #b45309; }

  .cm-item-detail {
    color: #6b7280;
    white-space: nowrap;
  }

  .cm-connect-btn {
    font-size: 12px;
    padding: 0.25rem 0.6rem;
  }

  .cm-edit-btn,
  .cm-delete-btn {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 15px;
    line-height: 1;
    padding: 2px 5px;
    border-radius: 4px;
    transition: color 0.15s, background 0.15s;
  }
  .cm-edit-btn       { color: #6b7280; }
  .cm-edit-btn:hover:not(:disabled)   { color: #2563eb; background: #eff6ff; }
  .cm-delete-btn     { color: #9ca3af; }
  .cm-delete-btn:hover:not(:disabled) { color: #ef4444; }
  .cm-edit-btn:disabled,
  .cm-delete-btn:disabled { opacity: 0.4; cursor: default; }

  .cm-delete-confirm {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .cm-delete-confirm-btn {
    font-size: 12px;
    padding: 0.2rem 0.5rem;
    background: #ef4444;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 600;
    transition: background 0.15s;
  }
  .cm-delete-confirm-btn:hover { background: #dc2626; }

  .cm-delete-cancel-btn {
    font-size: 12px;
    padding: 0.2rem 0.5rem;
  }

  /* ── Modal overlay ── */
  .cm-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0,0,0,0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .cm-dialog {
    background: white;
    border: 1px solid #e5e7eb;
    border-radius: 10px;
    padding: 0;
    width: min(480px, 96vw);
    max-height: 90vh;
    overflow-y: auto;
    box-shadow: 0 8px 32px rgba(0,0,0,0.18);
  }

  .cm-dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem 0.75rem;
    border-bottom: 1px solid #e5e7eb;
  }

  .cm-dialog-header h3 {
    margin: 0;
    font-size: 1rem;
    color: #111827;
  }

  .cm-close-btn {
    background: none;
    border: none;
    font-size: 20px;
    line-height: 1;
    color: #9ca3af;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
  }
  .cm-close-btn:hover { color: #111827; background: #f3f4f6; }

  /* ── Form (shared in modal) ── */
  .cm-form {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1rem 1.25rem;
  }

  .cm-field {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 14px;
    font-weight: 500;
    color: #374151;
  }

  .cm-field span { width: 80px; flex-shrink: 0; }

  .cm-field input,
  .cm-field select {
    flex: 1;
    padding: 0.4rem 0.6rem;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 14px;
    background: white;
  }

  .cm-field input:focus,
  .cm-field select:focus {
    outline: none;
    border-color: #2563eb;
    box-shadow: 0 0 0 3px rgba(37,99,235,0.12);
  }

  .cm-hint {
    margin: 0 0 0.25rem 1.4rem;
    font-size: 12px;
    color: #6b7280;
  }

  .cm-device-hint {
    margin-left: calc(80px + 0.75rem);
    margin-top: -0.35rem;
  }

  .cm-port-row {
    align-items: center;
  }

  .cm-toggle-label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0.5rem 0 0.25rem calc(80px + 0.75rem);
    font-size: 12px;
    color: #6b7280;
    cursor: pointer;
  }
  .cm-toggle-label input[type="checkbox"] {
    margin: 0;
  }

  .cm-field-inner {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex: 1;
  }
  .cm-field-inner span { width: 80px; flex-shrink: 0; }
  .cm-field-inner select,
  .cm-field-inner input {
    flex: 1;
    padding: 0.4rem 0.6rem;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 14px;
    background: white;
  }

  .cm-refresh-btn {
    padding: 0.4rem 0.5rem;
    font-size: 14px;
    flex-shrink: 0;
  }

  .cm-dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding-top: 0.5rem;
    border-top: 1px solid #e5e7eb;
    margin-top: 0.25rem;
  }
</style>

