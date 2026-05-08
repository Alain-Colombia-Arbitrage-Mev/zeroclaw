import { NavLink } from 'react-router-dom';
import { basePath } from '../../lib/basePath';
import {
  LayoutDashboard,
  MessageSquare,
  Wrench,
  Clock,
  Puzzle,
  Brain,
  Network,
  Settings,
  DollarSign,
  Activity,
  Stethoscope,
  Monitor,
  Mic,
  Users,
  Zap,
  ChevronsLeft,
  ChevronsRight,
  type LucideIcon,
} from 'lucide-react';
import { t } from '@/lib/i18n';

// ─── Nav structure ──────────────────────────────────────────────────
//
// Grouped into named sections so the sidebar reads as a real product
// surface, not a flat icon dump. Section titles stay terse + uppercase
// + tracked, and only render when the sidebar is expanded — collapsed
// sidebars use 1px dividers instead.

interface NavItem {
  to: string;
  icon: LucideIcon;
  labelKey: string;
}

interface NavSection {
  title: string;
  items: NavItem[];
}

const NAV_SECTIONS: NavSection[] = [
  {
    title: 'Workspace',
    items: [
      { to: '/', icon: LayoutDashboard, labelKey: 'nav.dashboard' },
      { to: '/agent', icon: MessageSquare, labelKey: 'nav.agent' },
      { to: '/jarvis', icon: Mic, labelKey: 'nav.jarvis' },
    ],
  },
  {
    title: 'Agents',
    items: [
      { to: '/orchestrator', icon: Zap, labelKey: 'nav.orchestrator' },
      { to: '/agents', icon: Users, labelKey: 'nav.agents' },
      { to: '/tools', icon: Wrench, labelKey: 'nav.tools' },
    ],
  },
  {
    title: 'Automate',
    items: [
      { to: '/cron', icon: Clock, labelKey: 'nav.cron' },
      { to: '/integrations', icon: Puzzle, labelKey: 'nav.integrations' },
      { to: '/canvas', icon: Monitor, labelKey: 'nav.canvas' },
    ],
  },
  {
    title: 'Knowledge',
    items: [
      { to: '/memory', icon: Brain, labelKey: 'nav.memory' },
      { to: '/knowledge', icon: Network, labelKey: 'nav.knowledge' },
    ],
  },
  {
    title: 'System',
    items: [
      { to: '/config', icon: Settings, labelKey: 'nav.config' },
      { to: '/cost', icon: DollarSign, labelKey: 'nav.cost' },
      { to: '/logs', icon: Activity, labelKey: 'nav.logs' },
      { to: '/doctor', icon: Stethoscope, labelKey: 'nav.doctor' },
    ],
  },
];

// ─── Nav item ───────────────────────────────────────────────────────

function SidebarNavItem({
  item,
  showLabel,
  showTooltip,
  onClick,
}: {
  item: NavItem;
  showLabel: boolean;
  showTooltip: boolean;
  onClick: () => void;
}) {
  const { to, icon: Icon, labelKey } = item;
  return (
    <NavLink
      key={to}
      to={to}
      end={to === '/'}
      onClick={onClick}
      className={({ isActive }) =>
        [
          'group relative flex items-center text-sm transition-colors duration-150',
          showLabel
            ? 'h-9 gap-3 pl-3 pr-3 rounded-md'
            : 'mx-auto my-0.5 h-9 w-9 justify-center rounded-md',
          isActive ? 'font-medium' : 'font-normal',
        ].join(' ')
      }
      style={({ isActive }) => ({
        color: isActive ? 'var(--pc-text-primary)' : 'var(--pc-text-muted)',
        background: isActive ? 'var(--pc-accent-glow)' : 'transparent',
      })}
    >
      {({ isActive }) => (
        <>
          {/* Left accent bar — visible only when active and expanded */}
          {showLabel && isActive && (
            <span
              aria-hidden
              className="absolute top-1 bottom-1 left-0 w-[3px] rounded-r"
              style={{ background: 'var(--pc-accent)' }}
            />
          )}
          {/* Active dot for collapsed mode */}
          {!showLabel && isActive && (
            <span
              aria-hidden
              className="absolute right-1 top-1 h-1 w-1 rounded-full"
              style={{ background: 'var(--pc-accent)' }}
            />
          )}
          <Icon
            className="h-[18px] w-[18px] shrink-0 transition-colors"
            style={{
              color: isActive ? 'var(--pc-accent)' : undefined,
            }}
          />
          {showLabel && (
            <span className="whitespace-nowrap tracking-tight">
              {t(labelKey)}
            </span>
          )}
          {showTooltip && (
            <span
              className="absolute left-full ml-3 px-2 py-1 rounded text-xs whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50"
              style={{
                background: 'var(--pc-bg-elevated)',
                color: 'var(--pc-text-primary)',
                border: '1px solid var(--pc-border)',
              }}
            >
              {t(labelKey)}
            </span>
          )}
        </>
      )}
    </NavLink>
  );
}

// ─── Section block ──────────────────────────────────────────────────

function SidebarSection({
  section,
  collapsed,
  onItemClick,
}: {
  section: NavSection;
  collapsed: boolean;
  onItemClick: () => void;
}) {
  return (
    <div className="mb-3 last:mb-0">
      {collapsed ? (
        <div
          className="mx-3 my-2"
          style={{
            height: '1px',
            background: 'var(--pc-border)',
            opacity: 0.5,
          }}
        />
      ) : (
        <p
          className="px-3 mb-1 text-[10px] font-medium uppercase"
          style={{
            color: 'var(--pc-text-faint)',
            letterSpacing: '0.12em',
          }}
        >
          {section.title}
        </p>
      )}
      <div className={collapsed ? 'space-y-0' : 'space-y-0.5 px-2'}>
        {section.items.map((item) => (
          <SidebarNavItem
            key={item.to}
            item={item}
            showLabel={!collapsed}
            showTooltip={collapsed}
            onClick={onItemClick}
          />
        ))}
      </div>
    </div>
  );
}

// ─── Sidebar shell ──────────────────────────────────────────────────

interface SidebarProps {
  open: boolean;
  onClose: () => void;
  collapsed: boolean;
  onCollapseToggle?: () => void;
}

export default function Sidebar({
  open,
  onClose,
  collapsed,
  onCollapseToggle,
}: SidebarProps) {
  return (
    <>
      {/* Mobile backdrop */}
      {open && (
        <div
          className="md:hidden fixed inset-0 z-40 bg-black/60 transition-opacity"
          onClick={onClose}
          onKeyDown={(e) => {
            if (e.key === 'Escape') onClose();
          }}
          role="button"
          tabIndex={-1}
          aria-label="Close menu"
        />
      )}

      {/* Desktop sidebar */}
      <aside
        className="hidden md:flex fixed top-0 left-0 h-screen flex-col border-r z-50 transition-[width] duration-200 ease-in-out"
        style={{
          background: 'var(--pc-bg-base)',
          borderColor: 'var(--pc-border)',
          width: collapsed ? '64px' : '240px',
        }}
        aria-label={collapsed ? 'Collapsed sidebar' : 'Main sidebar'}
      >
        <SidebarBrand collapsed={collapsed} />

        <nav
          className={`flex-1 overflow-y-auto overflow-x-hidden py-3 ${collapsed ? '' : ''}`}
          aria-label="Main navigation"
        >
          {NAV_SECTIONS.map((section) => (
            <SidebarSection
              key={section.title}
              section={section}
              collapsed={collapsed}
              onItemClick={onClose}
            />
          ))}
        </nav>

        <SidebarFooter
          collapsed={collapsed}
          onCollapseToggle={onCollapseToggle}
        />
      </aside>

      {/* Mobile sidebar */}
      <aside
        className={[
          'md:hidden fixed top-0 left-0 h-screen w-64 flex flex-col border-r z-50 transition-transform duration-200 ease-out',
          open ? 'translate-x-0' : '-translate-x-full',
        ].join(' ')}
        style={{
          background: 'var(--pc-bg-base)',
          borderColor: 'var(--pc-border)',
        }}
        aria-label="Mobile menu"
      >
        <SidebarBrand collapsed={false} />
        <nav
          className="flex-1 overflow-y-auto py-3"
          aria-label="Main navigation"
        >
          {NAV_SECTIONS.map((section) => (
            <SidebarSection
              key={section.title}
              section={section}
              collapsed={false}
              onItemClick={onClose}
            />
          ))}
        </nav>
        <SidebarFooter collapsed={false} mobile />
      </aside>
    </>
  );
}

// ─── Brand block ────────────────────────────────────────────────────

function SidebarBrand({ collapsed }: { collapsed: boolean }) {
  return (
    <div
      className="flex items-center border-b shrink-0 overflow-hidden"
      style={{
        borderColor: 'var(--pc-border)',
        height: '56px',
        padding: collapsed ? '0' : '0 14px',
        justifyContent: collapsed ? 'center' : 'flex-start',
        gap: collapsed ? '0' : '10px',
      }}
    >
      <img
        src={`${basePath}/_app/zeroclaw-trans.png`}
        alt="Octopus Labs"
        className="h-8 w-8 rounded-md object-cover shrink-0"
        onError={(e) => {
          e.currentTarget.style.display = 'none';
        }}
      />
      <span
        className="text-sm font-semibold tracking-tight whitespace-nowrap transition-opacity duration-150"
        style={{
          color: 'var(--pc-text-primary)',
          opacity: collapsed ? 0 : 1,
          pointerEvents: collapsed ? 'none' : 'auto',
        }}
      >
        Octopus Labs
      </span>
    </div>
  );
}

// ─── Footer with collapse toggle ────────────────────────────────────

function SidebarFooter({
  collapsed,
  onCollapseToggle,
  mobile,
}: {
  collapsed: boolean;
  onCollapseToggle?: () => void;
  mobile?: boolean;
}) {
  if (mobile) {
    return (
      <div
        className="border-t px-4 py-3 text-[10px] font-medium uppercase tracking-[0.12em]"
        style={{
          borderColor: 'var(--pc-border)',
          color: 'var(--pc-text-faint)',
        }}
      >
        Octopus Labs Runtime
      </div>
    );
  }
  if (!onCollapseToggle) return null;
  return (
    <div
      className="border-t shrink-0"
      style={{ borderColor: 'var(--pc-border)' }}
    >
      <button
        type="button"
        onClick={onCollapseToggle}
        aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        className="w-full flex items-center transition-colors group"
        style={{
          height: '40px',
          padding: collapsed ? '0' : '0 12px',
          justifyContent: collapsed ? 'center' : 'space-between',
          color: 'var(--pc-text-muted)',
          background: 'transparent',
          cursor: 'pointer',
          border: 'none',
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.background = 'var(--pc-hover)';
          e.currentTarget.style.color = 'var(--pc-text-primary)';
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.background = 'transparent';
          e.currentTarget.style.color = 'var(--pc-text-muted)';
        }}
      >
        {!collapsed && (
          <span
            className="text-xs font-medium"
            style={{ letterSpacing: '0.02em' }}
          >
            Collapse
          </span>
        )}
        {collapsed ? (
          <ChevronsRight className="h-[18px] w-[18px]" />
        ) : (
          <ChevronsLeft className="h-[18px] w-[18px]" />
        )}
      </button>
    </div>
  );
}
