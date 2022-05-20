const SECOND = 1000;

module.exports = {
  transform: {
    "^.+\\.(t|j)sx?$": ["@swc/jest"],
  },
  testTimeout: 60 * SECOND,
};
