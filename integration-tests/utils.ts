import urlJoin from "url-join";
import { decode } from "@msgpack/msgpack";
import { retryFetch } from "./fetch";

const CDN_ROOT: string = process.env.CDN_ROOT || "http://localhost:8080";
const CDN_VERSION = 4;

export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function encodePayload(payload) {
  return Buffer.from(`${CDN_VERSION}(${payload})`).toString("base64");
}

export type DepMap = { [depName: string]: string };
export interface IResolvedDependency {
  // name
  n: string;
  // version
  v: string;
  // depth
  d: number;
}

export async function fetchManifest(
  deps: DepMap
): Promise<IResolvedDependency[]> {
  const encoded_manifest = encodePayload(JSON.stringify(deps));
  const result = await retryFetch(
    urlJoin(CDN_ROOT, `/dep_tree/${encoded_manifest}`),
    {
      maxRetries: 5,
      retryDelay: 1000,
    }
  );
  return result.json();
}

export type CDNModuleFileType = ICDNModuleFile | number;

export interface ICDNModuleFile {
  // content
  c: string;
  // dependencies
  d: string[];
  // is transpiled
  t: boolean;
}

export interface ICDNModule {
  // files
  f: Record<string, CDNModuleFileType>;
  // transient dependencies
  m: string[];
}

export async function fetchModule(
  name: string,
  version: string
): Promise<ICDNModule> {
  const specifier = `${name}@${version}`;
  const encoded_specifier = encodePayload(specifier);
  const result = await retryFetch(
    urlJoin(CDN_ROOT, `/package/${encoded_specifier}`),
    { maxRetries: 5 }
  );
  return result.json();
}

export type V2Module = Record<string, Buffer>;

export async function fetchV2Module(
  name: string,
  version: string
): Promise<V2Module> {
  const specifier = `${name}@${version}`;
  const encoded_specifier = encodePayload(specifier);
  const result = await retryFetch(
    urlJoin(CDN_ROOT, `/v2/mod/${encoded_specifier}`),
    { maxRetries: 5 }
  );
  // @ts-ignore
  const blob = await result.buffer();
  return decode(blob) as V2Module;
}

export type V2Deps = Record<string, string>;

export async function fetchV2Deps(
  deps: Array<{name: string, range: string}>
): Promise<V2Deps> {
  const specifier = deps.map(v => `${v.name}@${v.range}`).join(';');
  const encoded_specifier = encodePayload(specifier);
  const url = urlJoin(CDN_ROOT, `/v2/deps/${encoded_specifier}`);
  const result = await retryFetch(
    url,
    { maxRetries: 5 }
  );
  // @ts-ignore
  const blob = await result.buffer();
  return decode(blob) as V2Deps;
}
