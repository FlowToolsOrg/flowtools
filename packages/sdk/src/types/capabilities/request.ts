/**
 * Primitive values allowed in URL query strings.
 */
export type RequestQueryValue = string | number | boolean

/**
 * HTTP request options for runtime networking capability.
 */
export interface RequestOptions {
  /**
   * Optional HTTP headers.
   */
  headers?: Record<string, string>
  /**
   * Optional query parameters appended to URL.
   */
  query?: Record<string, RequestQueryValue>
}

/**
 * Network capability contract.
 */
export interface RequestCapability {
  /**
   * Send an HTTP GET request.
   */
  get: <Out = unknown>(url: string, options?: RequestOptions) => Promise<Out>
  /**
   * Send an HTTP POST request with JSON-compatible body.
   */
  post: <Out = unknown>(
    url: string,
    body?: unknown,
    options?: RequestOptions
  ) => Promise<Out>
}
