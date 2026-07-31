import { create } from 'zustand';
import type { AppView } from '../types/domain';

interface AppState { view: AppView; sidebarOpen: boolean; inspectorOpen: boolean; focusMode: boolean; selectedEntityId: string | null; setView: (view: AppView) => void; toggleSidebar: () => void; toggleInspector: () => void; toggleFocusMode: () => void; selectEntity: (id: string | null) => void; }
export const useAppStore = create<AppState>((set) => ({ view: 'dashboard', sidebarOpen: true, inspectorOpen: true, focusMode: false, selectedEntityId: null, setView: (view) => set({ view }), toggleSidebar: () => set((state) => ({ sidebarOpen: !state.sidebarOpen })), toggleInspector: () => set((state) => ({ inspectorOpen: !state.inspectorOpen })), toggleFocusMode: () => set((state) => ({ focusMode: !state.focusMode })), selectEntity: (selectedEntityId) => set({ selectedEntityId }) }));
