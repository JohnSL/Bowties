type ShortcutAction = () => void;

type ShortcutGuards = {
  canOpenLayout: () => boolean;
  canCloseLayout: () => boolean;
  canSaveLayout: () => boolean;
  canSaveLayoutAs: () => boolean;
};

type ShortcutActions = {
  openLayout: ShortcutAction;
  closeLayout: ShortcutAction;
  saveLayout: ShortcutAction;
  saveLayoutAs: ShortcutAction;
};

type InstallMenuShortcutParams = {
  guards: ShortcutGuards;
  actions: ShortcutActions;
};

export function installMenuShortcuts(params: InstallMenuShortcutParams): () => void {
  const { guards, actions } = params;

  const onKeyDown = (event: KeyboardEvent) => {
    if (!(event.ctrlKey || event.metaKey)) return;
    if (event.altKey) return;

    const key = event.key.toLowerCase();

    if (key === 'o' && !event.shiftKey && guards.canOpenLayout()) {
      event.preventDefault();
      actions.openLayout();
      return;
    }

    if (key === 'w' && !event.shiftKey && guards.canCloseLayout()) {
      event.preventDefault();
      actions.closeLayout();
      return;
    }

    if (key === 's' && event.shiftKey && guards.canSaveLayoutAs()) {
      event.preventDefault();
      actions.saveLayoutAs();
      return;
    }

    if (key === 's' && !event.shiftKey && guards.canSaveLayout()) {
      event.preventDefault();
      actions.saveLayout();
    }
  };

  window.addEventListener('keydown', onKeyDown, true);
  return () => window.removeEventListener('keydown', onKeyDown, true);
}
