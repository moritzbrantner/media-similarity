import type { Preview } from "@storybook/react-vite";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router";
import "../frontend/src/styles.css";

type StorybookApiMock = {
  method?: string;
  response: unknown;
  status?: number;
  url: string;
};

const preview: Preview = {
  decorators: [
    (Story, context) => {
      installFetchMocks(context.parameters.apiMocks);

      const queryClient = new QueryClient({
        defaultOptions: {
          queries: {
            refetchOnWindowFocus: false,
            retry: false,
          },
        },
      });
      const route = typeof context.parameters.route === "string" ? context.parameters.route : "/";

      return (
        <QueryClientProvider client={queryClient}>
          <MemoryRouter initialEntries={[route]}>
            <div className="min-h-screen bg-neutral-100 p-6 text-neutral-950">
              <Story />
            </div>
          </MemoryRouter>
        </QueryClientProvider>
      );
    },
  ],
};

export default preview;

function installFetchMocks(parameter: unknown) {
  if (typeof window === "undefined" || !Array.isArray(parameter)) {
    return;
  }

  const mocks = parameter.filter(isApiMock);
  if (mocks.length === 0) {
    return;
  }

  window.fetch = async (input, init) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
    const method = (init?.method ?? "GET").toUpperCase();
    const mock = mocks.find(
      (item) => url.includes(item.url) && (item.method ?? "GET").toUpperCase() === method,
    );

    if (!mock) {
      return new Response(JSON.stringify({ detail: `No Storybook mock for ${method} ${url}` }), {
        headers: { "Content-Type": "application/json" },
        status: 404,
      });
    }

    return new Response(JSON.stringify(mock.response), {
      headers: { "Content-Type": "application/json" },
      status: mock.status ?? 200,
    });
  };
}

function isApiMock(value: unknown): value is StorybookApiMock {
  if (!value || typeof value !== "object") {
    return false;
  }

  const item = value as Partial<StorybookApiMock>;
  return typeof item.url === "string" && "response" in item;
}
