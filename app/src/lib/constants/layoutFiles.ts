export const OFFLINE_LAYOUT_EXTENSION = 'layout';
export const OFFLINE_LAYOUT_DEFAULT_FILENAME = `layout.${OFFLINE_LAYOUT_EXTENSION}`;

export function offlineLayoutDialogFilter(): { name: string; extensions: string[] } {
  return {
    name: 'Bowties Offline Layout',
    extensions: [OFFLINE_LAYOUT_EXTENSION],
  };
}