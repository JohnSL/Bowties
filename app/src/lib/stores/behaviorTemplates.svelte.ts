import { listBehaviorTemplates, type BehaviorTemplate } from '$lib/api/behaviorTemplates';

/**
 * Read-only mirror of the backend behavior template registry (spec 018).
 *
 * Application-scoped, not layout-scoped: templates are code-level in
 * `bowties_core::behavior_templates`, so the store is loaded once at app
 * start and is NOT enrolled in `layoutLifecycleOrchestrator.resetForNewLayout()`
 * (per ADR review D4 for S1).
 */
class BehaviorTemplatesStore {
  private _templates = $state<BehaviorTemplate[]>([]);
  private _loaded = $state(false);

  get templates(): BehaviorTemplate[] {
    return this._templates;
  }

  get loaded(): boolean {
    return this._loaded;
  }

  findByTemplateId(templateId: string): BehaviorTemplate | undefined {
    return this._templates.find((t) => t.templateId === templateId);
  }

  /** Load the registry from the backend. Idempotent. */
  async loadBehaviorTemplates(): Promise<void> {
    this._templates = await listBehaviorTemplates();
    this._loaded = true;
  }

  /** Used only by tests. Production never resets this store. */
  reset(): void {
    this._templates = [];
    this._loaded = false;
  }
}

export const behaviorTemplatesStore = new BehaviorTemplatesStore();
