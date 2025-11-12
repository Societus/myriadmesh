/**
 * Data service for polling and updating stores
 */

import * as api from './api';
import { nodeInfo, adapters, heartbeatStats, nodeMap, failoverEvents, isLoading, error } from './stores';

let pollingInterval: ReturnType<typeof setInterval> | null = null;

/**
 * Fetch all data and update stores
 */
export async function refreshData(): Promise<void> {
	try {
		isLoading.set(true);
		error.set(null);

		// Fetch all data in parallel
		const [nodeInfoData, adaptersData, heartbeatData, nodeMapData, eventsData] = await Promise.all([
			api.getNodeInfo().catch(() => null),
			api.getAdapters().catch(() => []),
			api.getHeartbeatStats().catch(() => null),
			api.getNodeMap().catch(() => []),
			api.getFailoverEvents().catch(() => [])
		]);

		// Update stores
		nodeInfo.set(nodeInfoData);
		adapters.set(adaptersData);
		heartbeatStats.set(heartbeatData);
		nodeMap.set(nodeMapData);
		failoverEvents.set(eventsData);

		isLoading.set(false);
	} catch (err) {
		console.error('Failed to refresh data:', err);
		error.set(err instanceof Error ? err.message : 'Unknown error');
		isLoading.set(false);
	}
}

/**
 * Start automatic polling
 */
export function startPolling(intervalMs: number = 5000): void {
	// Initial fetch
	refreshData();

	// Clear existing interval
	if (pollingInterval) {
		clearInterval(pollingInterval);
	}

	// Start new interval
	pollingInterval = setInterval(() => {
		refreshData();
	}, intervalMs);
}

/**
 * Stop automatic polling
 */
export function stopPolling(): void {
	if (pollingInterval) {
		clearInterval(pollingInterval);
		pollingInterval = null;
	}
}

/**
 * Update polling interval
 */
export function updatePollingInterval(intervalMs: number): void {
	startPolling(intervalMs);
}
