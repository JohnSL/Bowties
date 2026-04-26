<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher<{
    connected: {
      config: {
        id: string;
        name: string;
        adapterType: 'tcp';
        host: string;
        port: number;
      };
    };
  }>();

  function connect(): void {
    dispatch('connected', {
      config: {
        id: 'test-bus',
        name: 'Bench Bus',
        adapterType: 'tcp',
        host: '127.0.0.1',
        port: 12021,
      },
    });
  }
</script>

<button type="button" data-testid="connect-manager-button" on:click={connect}>
  Connect Test Bus
</button>