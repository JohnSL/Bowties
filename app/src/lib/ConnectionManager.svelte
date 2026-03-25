<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { createEventDispatcher, onMount } from 'svelte';

  type AdapterType = 'tcp' | 'gridConnectSerial' | 'slcanSerial';

  interface ConnectionConfig {
    id: string;
    name: string;
    adapterType: AdapterType;
    host?: string;
    port?: number;
    serialPort?: string;
    baudRate?: number;
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

  // Saved connections loaded from backend
  let savedConnections = $state<ConnectionConfig[]>([]);
  let sortedConnections = $derived([...savedConnections].sort((a, b) => a.name.localeCompare(b.name)));

  // Available serial ports
  let availablePorts = $state<string[]>([]);
  let portsLoading = $state(false);

  // Modal state
  let showModal = $state(false);
  let editingId = $state<string | null>(null); // null = new connection

  // Form fields (shared between add and edit)
  let formName = $state('');
  let formType = $state<AdapterType>('tcp');
  let formHost = $state('localhost');
  let formTcpPort = $state(12021);
  let formSerialPort = $state('');
  let formBaudRate = $state(57600);

  // Connection in progress
  let connectingId = $state<string | null>(null);
  let errorMessage = $state('');

  // Pending delete confirmation
  let confirmDeleteId = $state<string | null>(null);

  // Default baud rates per adapter type
  const defaultBaudRates: Record<AdapterType, number> = {
    tcp: 0,
    gridConnectSerial: 57600,
    slcanSerial: 115200,
  };

  // Update baud rate when type changes (only when adding, not while editing)
  $effect(() => {
    if (editingId === null && (formType === 'gridConnectSerial' || formType === 'slcanSerial')) {
      formBaudRate = defaultBaudRates[formType];
    }
  });

  onMount(async () => {
    await Promise.all([loadPrefs(), refreshPorts()]);
  });

  async function loadPrefs() {
    try {
      savedConnections = await invoke<ConnectionConfig[]>('load_connection_prefs');
    } catch (e) {
      console.error('Failed to load connection prefs:', e);
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

  async function savePrefs() {
    try {
      await invoke('save_connection_prefs', { connections: savedConnections });
    } catch (e) {
      console.error('Failed to save connection prefs:', e);
    }
  }

  function openAddModal() {
    editingId = null;
    formName = '';
    formType = 'tcp';
    formHost = 'localhost';
    formTcpPort = 12021;
    formSerialPort = availablePorts[0] ?? '';
    formBaudRate = defaultBaudRates['tcp'];
    errorMessage = '';
    showModal = true;
  }

  function openEditModal(conn: ConnectionConfig) {
    editingId = conn.id;
    formName = conn.name;
    formType = conn.adapterType;
    formHost = conn.host ?? 'localhost';
    formTcpPort = conn.port ?? 12021;
    formSerialPort = conn.serialPort ?? (availablePorts[0] ?? '');
    formBaudRate = conn.baudRate ?? defaultBaudRates[conn.adapterType];
    errorMessage = '';
    showModal = true;
  }

  function closeModal() {
    showModal = false;
  }

  async function submitForm() {
    if (!formName.trim()) return;

    try {
      const config: ConnectionConfig = {
        id: editingId ?? generateUUID(),
        name: formName.trim(),
        adapterType: formType,
      };

      if (formType === 'tcp') {
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
      await savePrefs();
      closeModal();
    } catch (e) {
      errorMessage = `Failed to save connection: ${e}`;
    }
  }

  async function deleteConnection(id: string) {
    savedConnections = savedConnections.filter(c => c.id !== id);
    confirmDeleteId = null;
    await savePrefs();
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
      case 'gridConnectSerial': return 'GridConnect';
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

        <!-- Type -->
        <fieldset class="cm-fieldset">
          <legend>Connection type</legend>

          <label class="cm-radio">
            <input type="radio" bind:group={formType} value="tcp" />
            <span class="cm-radio-label">TCP</span>
          </label>
          <p class="cm-hint">Connect via a network hub such as JMRI or a standalone TCP/IP bridge.</p>

          <label class="cm-radio">
            <input type="radio" bind:group={formType} value="gridConnectSerial" />
            <span class="cm-radio-label">GridConnect <em>(USB/Serial)</em></span>
          </label>
          <p class="cm-hint">Compatible devices: SPROG CANISB, SPROG USB-LCC, RR-Cirkits Buffer LCC, CAN2USBINO, CANRS</p>

          <label class="cm-radio">
            <input type="radio" bind:group={formType} value="slcanSerial" />
            <span class="cm-radio-label">SLCAN <em>(USB/Serial)</em></span>
          </label>
          <p class="cm-hint">Compatible devices: Canable, Lawicel CANUSB, any slcand-compatible adapter</p>
        </fieldset>

        <!-- TCP fields -->
        {#if formType === 'tcp'}
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
        {#if formType === 'gridConnectSerial' || formType === 'slcanSerial'}
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

          <details class="cm-advanced">
            <summary>Advanced</summary>
            <label class="cm-field">
              <span>Baud rate</span>
              <input type="number" bind:value={formBaudRate} min="1200" max="3000000" />
            </label>
          </details>
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

  .cm-fieldset {
    border: 1px solid #e5e7eb;
    border-radius: 6px;
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .cm-fieldset legend {
    font-size: 12px;
    font-weight: 600;
    color: #6b7280;
    padding: 0 4px;
  }

  .cm-radio {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    margin-top: 0.4rem;
  }
  .cm-radio-label { font-size: 14px; font-weight: 500; color: #111827; }
  .cm-radio-label em { font-style: normal; color: #6b7280; font-weight: 400; }

  .cm-hint {
    margin: 0 0 0.25rem 1.4rem;
    font-size: 12px;
    color: #6b7280;
  }

  .cm-port-row {
    align-items: center;
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

  .cm-advanced summary {
    font-size: 12px;
    color: #6b7280;
    cursor: pointer;
  }
  .cm-advanced .cm-field { margin-top: 0.5rem; }

  .cm-dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding-top: 0.5rem;
    border-top: 1px solid #e5e7eb;
    margin-top: 0.25rem;
  }
</style>

