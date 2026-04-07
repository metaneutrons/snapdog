"use client";

import { useEffect, useRef, useCallback, useState } from "react";
import type { WsNotification, WsCommand } from "@/lib/types";
import { getApiKey } from "@/lib/auth";

const MAX_BACKOFF = 30_000;

export function useWebSocket(onNotification: (n: WsNotification) => void, onReconnect?: () => void) {
  const [isConnected, setIsConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const backoffRef = useRef(1_000);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const onNotifRef = useRef(onNotification);
  onNotifRef.current = onNotification;

  const connect = useCallback(() => {
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    const key = getApiKey();
    const wsUrl = key
      ? `${proto}//${location.host}/ws?token=${encodeURIComponent(key)}`
      : `${proto}//${location.host}/ws`;
    const ws = new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      setIsConnected(true);
      backoffRef.current = 1_000;
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
      timerRef.current = setTimeout(() => {
        backoffRef.current = Math.min(backoffRef.current * 2, MAX_BACKOFF);
        connect();
      }, backoffRef.current);
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
