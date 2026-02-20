/**
 * Primitive values allowed in URL query strings.
 * @deprecated Use URLSearchParams + RequestInit directly.
 */
export type RequestQueryValue = string | number | boolean

/**
 * HTTP request options for runtime networking capability.
 * @deprecated Use RequestInit directly.
 */
export type RequestOptions = RequestInit

/**
 * Network capability contract with fetch-compatible signature.
 */
export type RequestCapability = (
  input: RequestInfo | URL,
  init?: RequestInit
) => Promise<Response>
