export async function parseResponse<T>(response: Response): Promise<T> {
  const text = await response.text();
  const payload = text ? tryParseJson(text) : null;

  if (!response.ok) {
    const parsedDetail = errorDetail(payload);
    const detail = parsedDetail ?? (text ? text : `${response.status} ${response.statusText}`);
    throw new Error(detail);
  }

  return payload as T;
}

export function tryParseJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

export function errorDetail(payload: unknown): string | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }

  if (!("detail" in payload)) {
    return null;
  }

  const detail = payload.detail;
  if (typeof detail === "string") {
    return detail;
  }

  if (Array.isArray(detail)) {
    return detail
      .map((item) => {
        if (
          item && typeof item === "object" && "msg" in item && typeof item.msg === "string"
        ) {
          return item.msg;
        }
        return JSON.stringify(item);
      })
      .join("; ");
  }

  return null;
}
