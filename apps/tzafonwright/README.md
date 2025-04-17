# Tzafonwright

TzafonWright is a Python library that provides a unified control interface for automation across different environments. It creates an abstraction layer that simplifies browser automation (via Playwright) using WebSocket commands, while maintaining performance and reliability. 

## Features

Traditional browser automation tools often suffer from flakiness and inconsistent behavior across environments. TzafonWright addresses these challenges by:

*   **Reducing Flakiness:** By abstracting away Playwright's implementation details, TzafonWright provides a more stable automation experience.
*   **Simplifying Migration:** The consistent interface makes it easier to migrate automation scripts between environments without extensive rewrites.
*   **Improving Benchmarking:** With reduced variability in test execution, you get more reliable performance metrics when benchmarking your applications.

## Requirements

*   Python >= 3.10

## Installation

You can install Tzafonwright using pip:

```bash
pip install ./apps/tzafonwright
```
