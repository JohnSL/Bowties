<script lang="ts">
  import DialogActions from './DialogActions.svelte';
  import Button from './Button.svelte';
</script>

{#snippet leading()}<Button>Cancel</Button>{/snippet}

<DialogActions {leading}>
  <Button>Back</Button>
  <Button>Next</Button>
</DialogActions>
