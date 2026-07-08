import {
  FolderSearch,
  Search,
  Settings,
  SlidersHorizontal,
  Users,
  type LucideIcon,
} from "lucide-react";

export type AppRouteId = "albums" | "registry" | "search" | "sources" | "workflows";

export type AppRouteNavItem = {
  icon: LucideIcon;
  id: AppRouteId;
  label: string;
  path: string;
  screenReaderLabel: string;
};

export const appRouteNavItems = [
  { icon: Search, id: "search", label: "Search", path: "/", screenReaderLabel: "Open query page" },
  {
    icon: FolderSearch,
    id: "albums",
    label: "Albums",
    path: "/albums",
    screenReaderLabel: "Open smart albums",
  },
  {
    icon: Users,
    id: "registry",
    label: "Registry",
    path: "/registry",
    screenReaderLabel: "Open inverse index",
  },
  {
    icon: Settings,
    id: "sources",
    label: "Sources",
    path: "/sources",
    screenReaderLabel: "Open media configuration",
  },
  {
    icon: SlidersHorizontal,
    id: "workflows",
    label: "Workflows",
    path: "/workflows",
    screenReaderLabel: "Open workflow editor",
  },
] as const satisfies readonly AppRouteNavItem[];
