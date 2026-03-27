/**
 * デバイス管理 API
 */

export interface Device {
  device_id: string
  display_name?: string
  last_seen_ts?: number
  last_seen_ip?: string
}

/** GET /_matrix/client/v3/devices */
export async function fetchDevices(homeserver: string, token: string): Promise<Device[]> {
  const res = await fetch(`${homeserver}/_matrix/client/v3/devices`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!res.ok) throw new Error(`devices failed: ${res.status}`)
  const data = (await res.json()) as { devices: Device[] }
  return data.devices
}

/** PUT /_matrix/client/v3/devices/{deviceId} */
export async function renameDevice(
  homeserver: string,
  token: string,
  deviceId: string,
  displayName: string
): Promise<void> {
  const res = await fetch(
    `${homeserver}/_matrix/client/v3/devices/${encodeURIComponent(deviceId)}`,
    {
      method: 'PUT',
      headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
      body: JSON.stringify({ display_name: displayName }),
    }
  )
  if (!res.ok) {
    const err = (await res.json().catch(() => ({}))) as { error?: string }
    throw new Error(err.error ?? `rename failed: ${res.status}`)
  }
}

/**
 * DELETE /_matrix/client/v3/devices/{deviceId}
 * Matrix 仕様では UIA が必要。currentPassword で m.login.password ステージを解決する。
 */
export async function deleteDevice(
  homeserver: string,
  token: string,
  userId: string,
  deviceId: string,
  currentPassword: string
): Promise<void> {
  const res = await fetch(
    `${homeserver}/_matrix/client/v3/devices/${encodeURIComponent(deviceId)}`,
    {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
      body: JSON.stringify({
        auth: {
          type: 'm.login.password',
          user: userId,
          password: currentPassword,
        },
      }),
    }
  )
  if (!res.ok) {
    const err = (await res.json().catch(() => ({}))) as { error?: string }
    throw new Error(err.error ?? `delete device failed: ${res.status}`)
  }
}

/**
 * POST /_matrix/client/v3/account/password
 * UIA: m.login.password ステージで現在のパスワードを認証する。
 */
export async function changePassword(
  homeserver: string,
  token: string,
  userId: string,
  currentPassword: string,
  newPassword: string
): Promise<void> {
  const res = await fetch(`${homeserver}/_matrix/client/v3/account/password`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({
      new_password: newPassword,
      auth: {
        type: 'm.login.password',
        user: userId,
        password: currentPassword,
      },
    }),
  })
  if (!res.ok) {
    const err = (await res.json().catch(() => ({}))) as { error?: string }
    throw new Error(err.error ?? `change password failed: ${res.status}`)
  }
}
