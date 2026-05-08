// Active tenant context — persists the operator's currently selected
// company in localStorage and pushes X-Octopus-Tenant on every API
// request via the apiFetch wrapper.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import {
  getTenants,
  type Tenant,
  type TenantCategory,
  type TenantStage,
} from '../lib/api';

const STORAGE_KEY = 'octopus_active_tenant';

interface TenantContextValue {
  active: Tenant | null;
  tenants: Tenant[];
  categories: TenantCategory[];
  stages: TenantStage[];
  loading: boolean;
  setActive: (id: string | null) => void;
  refresh: () => Promise<void>;
}

const TenantContext = createContext<TenantContextValue>({
  active: null,
  tenants: [],
  categories: [],
  stages: [],
  loading: true,
  setActive: () => {},
  refresh: async () => {},
});

export function TenantProvider({ children }: { children: ReactNode }) {
  const [tenants, setTenants] = useState<Tenant[]>([]);
  const [categories, setCategories] = useState<TenantCategory[]>([]);
  const [stages, setStages] = useState<TenantStage[]>([]);
  const [activeId, setActiveId] = useState<string | null>(() => {
    try {
      return localStorage.getItem(STORAGE_KEY);
    } catch {
      return null;
    }
  });
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const r = await getTenants();
      setTenants(r.tenants);
      setCategories(r.categories);
      setStages(r.stages);
    } catch {
      // Pairing not yet established or daemon down — fail soft
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Persist + propagate active tenant id to fetch wrapper via a custom
  // event on window so the api.ts module can pick it up without a
  // hard import cycle.
  useEffect(() => {
    try {
      if (activeId) localStorage.setItem(STORAGE_KEY, activeId);
      else localStorage.removeItem(STORAGE_KEY);
    } catch {
      /* ignore */
    }
    // Notify the fetch wrapper
    window.dispatchEvent(
      new CustomEvent('octopus:tenant-change', { detail: activeId }),
    );
  }, [activeId]);

  // Auto-select first tenant if none selected
  useEffect(() => {
    if (!activeId && tenants.length > 0) {
      setActiveId(tenants[0]!.id);
    }
    // If the saved id no longer exists, clear it
    if (activeId && tenants.length > 0 && !tenants.some((t) => t.id === activeId)) {
      setActiveId(tenants[0]!.id);
    }
  }, [tenants, activeId]);

  const active = useMemo(
    () => tenants.find((t) => t.id === activeId) ?? null,
    [tenants, activeId],
  );

  const value: TenantContextValue = {
    active,
    tenants,
    categories,
    stages,
    loading,
    setActive: setActiveId,
    refresh,
  };

  return <TenantContext.Provider value={value}>{children}</TenantContext.Provider>;
}

export function useTenant(): TenantContextValue {
  return useContext(TenantContext);
}
