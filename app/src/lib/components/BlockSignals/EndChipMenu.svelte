<script lang="ts">
  /**
   * EndChipMenu — what lies at one end of the track. Opened from an end chip
   * on the Blocks diagram. Interlocking Plants are not available yet, so
   * "Choose a Plant…" is shown disabled.
   */
  import type { EntranceEnd, ExitEnd } from '$lib/api/signaling';

  let {
    end,
    edgeName,
    entranceEnd,
    exitEnd,
    top,
    left,
    onEntranceChange,
    onExitChange,
    onClose,
  }: {
    end: 'start' | 'end';
    /** The block next to this end. */
    edgeName: string;
    entranceEnd: EntranceEnd;
    exitEnd: ExitEnd;
    top: number;
    left: number;
    onEntranceChange: (value: EntranceEnd) => void;
    onExitChange: (value: ExitEnd) => void;
    onClose: () => void;
  } = $props();

  type Option = { value: 'noPlant' | 'nothingBeyond' | 'treatAsStop'; title: string; description: string };
  const options = $derived<Option[]>(end === 'start'
    ? [{ value: 'noPlant', title: 'No Plant', description: 'Nothing before the first block reads these signals.' }]
    : [
      { value: 'nothingBeyond', title: 'Nothing beyond', description: 'The last signal shows Stop / Clear.' },
      { value: 'treatAsStop', title: 'Nothing beyond · treat as Stop', description: 'For staging or the end of signaled track. The last signal shows Approach at best.' },
    ]);
  const current = $derived(end === 'start' ? entranceEnd.kind : exitEnd.kind);

  let menu = $state<HTMLElement | null>(null);

  function choose(value: Option['value']): void {
    if (value === 'noPlant') onEntranceChange({ kind: 'noPlant' });
    else onExitChange({ kind: value });
  }

  $effect(() => {
    menu?.querySelector<HTMLInputElement>('input:checked, input')?.focus();
    const onPointer = (event: PointerEvent) => {
      const target = event.target as Element;
      if (menu && !menu.contains(target) && !target.closest?.('.track-end-chip')) onClose();
    };
    document.addEventListener('pointerdown', onPointer, true);
    return () => document.removeEventListener('pointerdown', onPointer, true);
  });
</script>

<div
  bind:this={menu}
  class="end-menu"
  role="dialog"
  aria-label={end === 'start' ? `What lies before ${edgeName}` : `What lies beyond ${edgeName}`}
  style:top={`${top}px`}
  style:left={`${left}px`}
  tabindex="-1"
  onkeydown={(event) => {
    if (event.key === 'Escape') {
      event.stopPropagation();
      onClose();
    }
  }}
>
  <span class="end-menu-heading">{end === 'start' ? `Before ${edgeName}` : `Beyond ${edgeName}`}</span>
  <div role="radiogroup" aria-label={end === 'start' ? 'Before the first block' : 'Beyond the last block'}>
    {#each options as option (option.value)}
      <label class="end-menu-choice" class:selected={current === option.value}>
        <input
          type="radio"
          name={`end-menu-${end}`}
          checked={current === option.value}
          onchange={() => choose(option.value)}
        />
        <span>
          <span class="end-menu-title">{option.title}</span>
          <span class="end-menu-desc">{option.description}</span>
        </span>
      </label>
    {/each}
  </div>
  <button type="button" class="end-menu-plant" disabled>Choose a Plant…</button>
  <span class="end-menu-desc">Interlocking Plants are coming in a later release.</span>
</div>

<style>
  .end-menu {
    position: absolute;
    z-index: 8;
    width: 300px;
    padding: 10px 12px;
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 8px;
    background: var(--fluent-neutralBackground1);
    box-shadow: 0 6px 20px rgba(0, 0, 0, .18);
  }
  .end-menu-heading { display: block; margin-bottom: 6px; font-size: 12px; font-weight: 600; color: var(--fluent-neutralForeground2); }
  .end-menu-choice {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    margin-top: 6px;
    padding: 8px 10px;
    border: 1px solid var(--fluent-neutralStroke2);
    border-radius: 6px;
    cursor: pointer;
  }
  .end-menu-choice.selected { border-color: var(--fluent-brandBackground); background: rgba(15, 108, 189, .08); }
  .end-menu-choice input { margin-top: 2px; }
  .end-menu-title { display: block; font-size: 13px; font-weight: 600; color: var(--fluent-neutralForeground1); }
  .end-menu-desc { display: block; margin-top: 2px; font-size: 11px; color: var(--fluent-neutralForeground3); }
  .end-menu-plant {
    margin-top: 10px;
    padding: 4px 12px;
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 4px;
    background: var(--fluent-neutralBackground1);
    color: var(--fluent-neutralForegroundDisabled, #a0a0a0);
    font-family: var(--fluent-fontFamily);
    font-size: 13px;
  }
</style>
