<script lang="ts">
  import type { ConnectionStatus, SNIPStatus } from '$lib/api/tauri';

  interface Props {
    connectionStatus: ConnectionStatus;
    snipStatus: SNIPStatus;
  }

  let { connectionStatus, snipStatus }: Props = $props();

  // Determine the status indicator color and text
  const statusInfo = $derived.by(() => {
    // Connection status takes priority
    if (connectionStatus === 'Connected') {
      return {
        color: 'bg-green-500',
        label: 'Connected',
        title: 'Node is online and responding'
      };
    } else if (connectionStatus === 'NotResponding') {
      return {
        color: 'bg-red-500',
        label: 'Not Responding',
        title: 'Node is not responding to verification'
      };
    } else if (connectionStatus === 'Verifying') {
      return {
        color: 'bg-yellow-500',
        label: 'Verifying',
        title: 'Checking node connection status'
      };
    }

    // If connection unknown, show SNIP status
    if (snipStatus === 'InProgress') {
      return {
        color: 'bg-blue-500',
        label: 'Loading SNIP',
        title: 'Retrieving node information'
      };
    } else if (snipStatus === 'Complete') {
      return {
        color: 'bg-green-500',
        label: 'Ready',
        title: 'Node information available'
      };
    } else if (snipStatus === 'Timeout') {
      return {
        color: 'bg-orange-500',
        label: 'Timeout',
        title: 'SNIP request timed out'
      };
    } else if (snipStatus === 'Error') {
      return {
        color: 'bg-red-500',
        label: 'Error',
        title: 'Error retrieving node information'
      };
    } else if (snipStatus === 'NotSupported') {
      return {
        color: 'bg-gray-400',
        label: 'No SNIP',
        title: 'Node does not support SNIP protocol'
      };
    }

    // Default: Unknown
    return {
      color: 'bg-gray-300',
      label: 'Unknown',
      title: 'Node status unknown'
    };
  });
</script>

<div class="flex items-center gap-2">
  <div 
    class="w-3 h-3 rounded-full {statusInfo.color}"
    title={statusInfo.title}
    aria-label={statusInfo.label}
    role="status"
  ></div>
  <span class="text-sm text-gray-600 dark:text-gray-400" title={statusInfo.title}>
    {statusInfo.label}
  </span>
</div>

<style>
  /* Optional: Add pulse animation for in-progress states */
  .bg-blue-500, .bg-yellow-500 {
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }
</style>
