<script lang="ts">
  interface Props {
    text?: string;
  }

  let { text = 'Not captured' }: Props = $props();
</script>

<span class="missing-capture-badge" title="This value was not present in the captured snapshot">
  {text}
</span>

<style>
  .missing-capture-badge {
    display: inline-flex;
    align-items: center;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    color: #991b1b;
    background: #fee2e2;
    border: 1px solid #fecaca;
  }
</style>
