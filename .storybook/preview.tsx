import type { Preview } from "@storybook/react-vite";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router";
import "../frontend/src/styles.css";

const preview: Preview = {
  decorators: [
    (Story, context) => {
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
