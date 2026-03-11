import React, { useState, useRef, useEffect, MouseEvent as ReactMouseEvent } from 'react';
import MDEditor from '@uiw/react-md-editor';
import { X, Code, Eye, SplitSquareHorizontal } from 'lucide-react';
import { useAppStore } from '../../store/appStore';

export function NoteFloatingWindow() {
    const isNotesWindowOpen = useAppStore(s => s.isNotesWindowOpen);
    const toggleNotesWindow = useAppStore(s => s.toggleNotesWindow);
    const activeTabId = useAppStore(s => s.activeTabId);
    const activeUi = useAppStore(s => s.activeTabId ? s.tabUi[s.activeTabId] : null);
    const updateTabUi = useAppStore(s => s.updateTabUi);

    const [mode, setMode] = useState<'edit' | 'preview' | 'live'>('live');
    
    // Position state initialized to center of screen
    const [position, setPosition] = useState({ 
        x: Math.max(0, window.innerWidth / 2 - 260), 
        y: Math.max(0, window.innerHeight / 2 - 190) 
    });
    
    const isDraggingRef = useRef(false);
    const dragStartPosRef = useRef({ x: 0, y: 0 });
    const dragStartWindowPosRef = useRef({ x: 0, y: 0 });

    useEffect(() => {
        // Initial centering (only needed if dimensions change drastically or on first calc issue)
        const updateCenter = () => {
             // Let it just rely on initial render to prevent jumping when resizing window
        };
        window.addEventListener('resize', updateCenter);
        return () => window.removeEventListener('resize', updateCenter);
    }, []);

    const handleMouseDown = (e: ReactMouseEvent<HTMLDivElement>) => {
        if (e.target instanceof HTMLElement && e.target.closest('.no-drag')) {
            return;
        }
        isDraggingRef.current = true;
        dragStartPosRef.current = { x: e.clientX, y: e.clientY };
        dragStartWindowPosRef.current = { ...position };
        
        document.body.style.userSelect = 'none'; // Prevent text selection during drag
    };

    useEffect(() => {
        const handleMouseMove = (e: MouseEvent) => {
            if (!isDraggingRef.current) return;
            
            const dx = e.clientX - dragStartPosRef.current.x;
            const dy = e.clientY - dragStartPosRef.current.y;
            
            setPosition({
                x: dragStartWindowPosRef.current.x + dx,
                y: dragStartWindowPosRef.current.y + dy
            });
        };

        const handleMouseUp = () => {
            isDraggingRef.current = false;
            document.body.style.userSelect = '';
        };

        if (isNotesWindowOpen) {
            document.addEventListener('mousemove', handleMouseMove);
            document.addEventListener('mouseup', handleMouseUp);
        }

        return () => {
            document.removeEventListener('mousemove', handleMouseMove);
            document.removeEventListener('mouseup', handleMouseUp);
        };
    }, [isNotesWindowOpen]);

    if (!isNotesWindowOpen) return null;

    const notesContent = activeUi?.viewNotes || "";

    const handleNotesChange = (val?: string) => {
        if (activeTabId) {
            updateTabUi(activeTabId, { viewNotes: val || "" });
        }
    };

    return (
        <div 
            className="fixed z-50 flex flex-col bg-zinc-950 border border-zinc-800 rounded-lg shadow-2xl overflow-hidden animate-fade-in"
            style={{ 
                left: `${position.x}px`, 
                top: `${position.y}px`,
                width: '520px',
                height: '380px',
                resize: 'both',
                minWidth: '300px',
                minHeight: '200px'
            }}
        >
            {/* Header / Drag Handle */}
            <div 
                className="flex items-center justify-between px-3 py-2 bg-zinc-900 border-b border-zinc-800 cursor-move text-zinc-300 select-none"
                onMouseDown={handleMouseDown}
            >
                <div className="flex items-center gap-2 font-medium text-sm">
                    Notes
                </div>
                
                <div className="flex items-center gap-1 no-drag">
                    <div className="flex bg-zinc-950 border border-zinc-800 rounded-md p-0.5 mr-2">
                        <button 
                            className={`p-1 rounded-sm ${mode === 'edit' ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-500 hover:text-zinc-300'}`}
                            onClick={() => setMode('edit')}
                            title="Code"
                        >
                            <Code size={14} />
                        </button>
                        <button 
                            className={`p-1 rounded-sm ${mode === 'preview' ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-500 hover:text-zinc-300'}`}
                            onClick={() => setMode('preview')}
                            title="Preview"
                        >
                            <Eye size={14} />
                        </button>
                        <button 
                            className={`p-1 rounded-sm ${mode === 'live' ? 'bg-zinc-800 text-zinc-100' : 'text-zinc-500 hover:text-zinc-300'}`}
                            onClick={() => setMode('live')}
                            title="Both"
                        >
                            <SplitSquareHorizontal size={14} />
                        </button>
                    </div>
                    
                    <button 
                        className="p-1 hover:bg-zinc-800 rounded-md text-zinc-400 hover:text-red-400 transition-colors"
                        onClick={toggleNotesWindow}
                    >
                        <X size={16} />
                    </button>
                </div>
            </div>

            {/* Content Body */}
            <div className="flex-1 overflow-hidden" data-color-mode="dark">
                {activeTabId ? (
                    <MDEditor
                        value={notesContent}
                        onChange={handleNotesChange}
                        preview={mode}
                        height="100%"
                        className="h-full w-full border-none rounded-none !bg-zinc-950"
                        hideToolbar={false}
                        visibleDragbar={false}
                    />
                ) : (
                    <div className="flex items-center justify-center h-full text-zinc-600 text-sm">
                        No active view to attach notes.
                    </div>
                )}
            </div>
        </div>
    );
}
