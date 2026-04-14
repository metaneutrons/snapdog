"use client";

import { useEffect, useRef, useCallback, useState } from "react";
import type { WsNotification, WsCommand } from "@/lib/types";
import { getApiKey } from "@/lib/auth";

const BACKOFF_STEPS = [1_000, 2_500, 5_000, 10_000, 15_000];

export function useWebSocket(onNotification: (n: WsNotification) => void, onReconnect?: () => void) {
  const [isConnected, setIsConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const attemptRef = useRef(0);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const onNotifRef = useRef(onNotification);
  const onReconnectRef = useRef(onReconnect);
  onNotifRef.current = onNotification;
  onReconnectRef.current = onReconnect;
  const wasConnectedRef = useRef(false);

  const connect = useCallback(() => {
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    const key = getApiKey();
    const wsUrl = key
      ? `${proto}//${location.host}/ws?token=${encodeURIComponent(key)}`
      : `${proto}//${location.host}/ws`;
    const ws = new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      const wasDisconnected = wasConnectedRef.current;
      setIsConnected(true);
      wasConnectedRef.current = true;
      attemptRef.current = 0;
      if (wasDisconnected) onReconnectRef.current?.();
    };

    ws.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data) as WsNotification;
        onNotifRef.current(data);
      } catch {
        /* ignore malformed messages */
      }
    };

    ws.onclose = () => {
      setIsConnected(false);
      wsRef.current = null;
      const delay = BACKOFF_STEPS[Math.min(attemptRef.current, BACKOFF_STEPS.length - 1)];
      attemptRef.current++;
      timerRef.current = setTimeout(connect, delay);
    };

    ws.onerror = () => ws.close();
  }, []);

  useEffect(() => {
    connect();
    return () => {
      clearTimeout(timerRef.current);
      wsRef.current?.close();
    };
  }, [connect]);

  const sendCommand = useCallback((zone: number, action: string, value?: WsCommand["value"]) => {
    const ws = wsRef.current;
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ zone, action, value }));
    }
  }, []);

  return { isConnected, sendCommand };
}
