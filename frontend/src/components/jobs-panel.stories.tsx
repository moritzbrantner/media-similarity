import type { Meta, StoryObj } from "@storybook/react-vite";
import { completedIndexEvents, completedIndexJob, makeJob } from "../testing/media-fixtures";
import { JobsPanel } from "./jobs-panel";

const activeJob = makeJob({
  finished_at: null,
  progress: {
    completed: 12,
    message: "indexed 12/40 pending source files",
    total: 40,
    unit: "files",
  },
  status: "Running",
});

const meta = {
  component: JobsPanel,
  args: {
    cancelPendingJobId: null,
    error: null,
    events: completedIndexEvents,
    jobs: [completedIndexJob],
    onCancel: () => undefined,
    onSelectJob: () => undefined,
    selectedJobId: completedIndexJob.spec.id,
  },
} satisfies Meta<typeof JobsPanel>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Completed: Story = {};

export const Active: Story = {
  args: {
    jobs: [activeJob, completedIndexJob],
    selectedJobId: activeJob.spec.id,
  },
};

export const Empty: Story = {
  args: {
    events: [],
    jobs: [],
    selectedJobId: null,
  },
};

export const ErrorState: Story = {
  args: {
    error: new Error("Unable to load background jobs."),
  },
};
