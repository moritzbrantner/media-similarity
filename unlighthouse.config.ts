export default {
  cache: false,
  ci: {
    budget: {
      performance: 50,
    },
    buildStatic: true,
  },
  lighthouseOptions: {
    onlyCategories: ["performance"],
  },
  outputPath: ".unlighthouse",
  scanner: {
    device: "desktop",
    samples: 1,
    throttle: false,
  },
  urls: ["/"],
};
