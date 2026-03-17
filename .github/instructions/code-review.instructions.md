---
description: 'Generic code review instructions that can be customized for any project using GitHub Copilot'
applyTo: '**'
excludeAgent: ["coding-agent"]
---

# Generic Code Review Instructions

Comprehensive code review guidelines for GitHub Copilot that can be adapted to any project. These instructions follow best practices from prompt engineering and provide a structured approach to code quality, security, testing, and architecture review.

## Review Language

When performing a code review, respond in **English**

## Review Priorities

When performing a code review, prioritize issues in the following order:

### 🔴 CRITICAL (Block merge)
- **Security**: Vulnerabilities, exposed secrets, authentication/authorization issues
- **Correctness**: Logic errors, data corruption risks, race conditions
- **Breaking Changes**: API contract changes without versioning
- **Data Loss**: Risk of data loss or corruption

### 🟡 IMPORTANT (Requires discussion)
- **Code Quality**: Severe violations of SOLID principles, excessive duplication
- **Test Coverage**: Missing tests for critical paths or new functionality
- **Performance**: Obvious performance bottlenecks (N+1 queries, memory leaks)
- **Architecture**: Significant deviations from established patterns

### 🟢 SUGGESTION (Non-blocking improvements)
- **Readability**: Poor naming, complex logic that could be simplified
- **Optimization**: Performance improvements without functional impact
- **Best Practices**: Minor deviations from conventions
- **Documentation**: Missing or incomplete comments/documentation

## General Review Principles

When performing a code review, follow these principles:

1. **Always use `suggestion` blocks**: Every comment that proposes a code change must include a ` ```suggestion ` fenced code block so the author can apply it directly from the PR via the "Commit suggestion" button. This is the single most important formatting rule.
2. **Be specific**: Reference exact lines, files, and provide concrete examples
3. **Provide context**: Explain WHY something is an issue and the potential impact
4. **Suggest solutions**: Show corrected code inside a `suggestion` block, not just what's wrong
5. **Be constructive**: Focus on improving the code, not criticizing the author
6. **Recognize good practices**: Acknowledge well-written code and smart solutions
7. **Be pragmatic**: Not every suggestion needs immediate implementation
8. **Group related comments**: Avoid multiple comments about the same topic

## Code Quality Standards
<formatting>

When performing a code review, check for:

### Clean Code
- Descriptive and meaningful names for variables, functions, and classes
- Single Responsibility Principle: each function/class does one thing well
- DRY (Don't Repeat Yourself): no code duplication
- Functions should be small and focused (ideally < 20-30 lines)
- Avoid deeply nested code (max 3-4 levels)
- Avoid magic numbers and strings (use constants)
- Code should be self-documenting; comments only when necessary
</formatting>

### Examples
```javascript
// BAD: Poor naming and magic numbers
function calc(x, y) {
    if (x > 100) return y * 0.15;
    return y * 0.10;
}
// GOOD: Clear naming and constants
const PREMIUM_THRESHOLD = 100;
const PREMIUM_DISCOUNT_RATE = 0.15;
const STANDARD_DISCOUNT_RATE = 0.10;
function calculateDiscount(orderTotal, itemPrice) {
    const isPremiumOrder = orderTotal > PREMIUM_THRESHOLD;
    const discountRate = isPremiumOrder ? PREMIUM_DISCOUNT_RATE : STANDARD_DISCOUNT_RATE;
    return itemPrice * discountRate;
}
```

### Error Handling
- Proper error handling at appropriate levels
- Meaningful error messages
- No silent failures or ignored exceptions
- Fail fast: validate inputs early
- Use appropriate error types/exceptions

### Examples
```python
# BAD: Silent failure and generic error
def process_user(user_id):
    try:
        user = db.get(user_id)
        user.process()
    except:
        pass
# GOOD: Explicit error handling
def process_user(user_id):
    if not user_id or user_id <= 0:
        raise ValueError(f"Invalid user_id: {user_id}")
    try:
        user = db.get(user_id)
    except UserNotFoundError:
        raise UserNotFoundError(f"User {user_id} not found in database")
    except DatabaseError as e:
        raise ProcessingError(f"Failed to retrieve user {user_id}: {e}")
    return user.process()
```

## Security Review

When performing a code review, check for security issues:

- **Sensitive Data**: No passwords, API keys, tokens, or PII in code or logs
- **Input Validation**: All user inputs are validated and sanitized
- **SQL Injection**: Use parameterized queries, never string concatenation
- **Authentication**: Proper authentication checks before accessing resources
- **Authorization**: Verify user has permission to perform action
- **Cryptography**: Use established libraries, never roll your own crypto
- **Dependency Security**: Check for known vulnerabilities in dependencies

### Examples
<examples>

```javascript
// ❌ BAD: Exposed secret in code
const API_KEY = "sk_live_abc123xyz789";
// ✅ GOOD: Use environment variables
const API_KEY = process.env.API_KEY;
```
</examples>

## Testing Standards

When performing a code review, verify test quality:

- **Coverage**: Critical paths and new functionality must have tests
- **Test Names**: Descriptive names that explain what is being tested
- **Test Structure**: Clear Arrange-Act-Assert or Given-When-Then pattern
- **Independence**: Tests should not depend on each other or external state
- **Assertions**: Use specific assertions, avoid generic assertTrue/assertFalse
- **Edge Cases**: Test boundary conditions, null values, empty collections
- **Mock Appropriately**: Mock external dependencies, not domain logic

### Examples
<examples>

```typescript
// ❌ BAD: Vague name and assertion
test('test1', () => {
    const result = calc(5, 10);
    expect(result).toBeTruthy();
});

// ✅ GOOD: Descriptive name and specific assertion
test('should calculate 10% discount for orders under $100', () => {
    const orderTotal = 50;
    const itemPrice = 20;

    const discount = calculateDiscount(orderTotal, itemPrice);

    expect(discount).toBe(2.00);
});
```
</examples>

## Performance Considerations

When performing a code review, check for performance issues:

- **Database Queries**: Avoid N+1 queries, use proper indexing
- **Algorithms**: Appropriate time/space complexity for the use case
- **Caching**: Utilize caching for expensive or repeated operations
- **Resource Management**: Proper cleanup of connections, files, streams
- **Pagination**: Large result sets should be paginated
- **Lazy Loading**: Load data only when needed

### Examples
<examples>

```python
# ❌ BAD: N+1 query problem
users = User.query.all()
for user in users:
    orders = Order.query.filter_by(user_id=user.id).all()  # N+1!

# ✅ GOOD: Use JOIN or eager loading
users = User.query.options(joinedload(User.orders)).all()
for user in users:
    orders = user.orders
```
</examples>

## Architecture and Design

When performing a code review, verify architectural principles:

- **Separation of Concerns**: Clear boundaries between layers/modules
- **Dependency Direction**: High-level modules don't depend on low-level details
- **Interface Segregation**: Prefer small, focused interfaces
- **Loose Coupling**: Components should be independently testable
- **High Cohesion**: Related functionality grouped together
- **Consistent Patterns**: Follow established patterns in the codebase

## Documentation Standards

When performing a code review, check documentation:

- **API Documentation**: Public APIs must be documented (purpose, parameters, returns)
- **Complex Logic**: Non-obvious logic should have explanatory comments
- **README Updates**: Update README when adding features or changing setup
- **Breaking Changes**: Document any breaking changes clearly
- **Examples**: Provide usage examples for complex features

## Comment Format Template

When performing a code review, **every comment that suggests a code change MUST include a GitHub suggestion block** so the author can apply the fix directly from the PR using the "Commit suggestion" button. This is non-negotiable.

A GitHub suggestion block uses the special `suggestion` language tag inside a fenced code block. GitHub renders this as a one-click "Commit suggestion" button on the PR. The content inside the block **replaces** the lines the comment is attached to.

### Rules for suggestion blocks

1. **Always use ` ```suggestion ` blocks** for any comment that proposes a code change, no matter how small (renames, formatting, logic changes, security fixes, etc.)
2. **One suggestion block per comment** - if you need to change multiple locations, leave separate review comments on each location
3. The suggestion block must contain the **complete replacement code** for the lines the comment targets - not a diff, not a before/after comparison, just the corrected code
4. **Do not include unchanged surrounding code** unless it is part of the lines the comment is attached to
5. If a comment is purely observational and does not propose a code change (e.g., asking a question, requesting clarification), a suggestion block is not needed
6. **Multi-line suggestions**: When the fix spans multiple lines, attach the review comment to the full line range and put all corrected lines in a single suggestion block

### Comment structure

```markdown
**[PRIORITY] Category: Brief title**

Why this matters: [1-2 sentence explanation of the impact]

```suggestion
corrected code here
```
```

### Example Comments

#### Critical Issue
```markdown
**🔴 CRITICAL - Security: SQL Injection Vulnerability**

The query concatenates user input directly into SQL, allowing an attacker to execute
arbitrary commands and expose or delete database data.

```suggestion
PreparedStatement stmt = conn.prepareStatement(
    "SELECT * FROM users WHERE email = ?"
);
stmt.setString(1, email);
```
```

#### Important Issue
```markdown
**🟡 IMPORTANT - Correctness: Unhandled null from findUser()**

`findUser()` can return null but the result is used without a check, which will
throw a TypeError at runtime.

```suggestion
const user = await findUser(id);
if (!user) {
    throw new NotFoundError(`User ${id} not found`);
}
return user.name;
```
```

#### Suggestion
```markdown
**🟢 SUGGESTION - Readability: Simplify nested conditionals with guard clause**

Deeply nested ifs are harder to maintain and test. A guard clause makes the
happy path clearer.

```suggestion
if (!user || !user.isActive || !user.hasPermission('write')) {
    return;
}
// do something
```
```

#### Multi-line rename/refactor
```markdown
**🟢 SUGGESTION - Readability: Use descriptive names and named constants**

Magic numbers and single-letter variable names make business logic opaque.

```suggestion
const PREMIUM_THRESHOLD = 100;
const PREMIUM_DISCOUNT_RATE = 0.15;
const STANDARD_DISCOUNT_RATE = 0.10;

function calculateDiscount(orderTotal, itemPrice) {
    const discountRate = orderTotal > PREMIUM_THRESHOLD
        ? PREMIUM_DISCOUNT_RATE
        : STANDARD_DISCOUNT_RATE;
    return itemPrice * discountRate;
}
```
```

## Review Checklist

When performing a code review, systematically verify:

### Code Quality
- [ ] Code follows consistent style and conventions
- [ ] Names are descriptive and follow naming conventions
- [ ] Functions/methods are small and focused
- [ ] No code duplication
- [ ] Complex logic is broken into simpler parts
- [ ] Error handling is appropriate
- [ ] No commented-out code or TODO without tickets

### Security
- [ ] No sensitive data in code or logs
- [ ] Input validation on all user inputs
- [ ] No SQL injection vulnerabilities
- [ ] Authentication and authorization properly implemented
- [ ] Dependencies are up-to-date and secure

### Testing
- [ ] New code has appropriate test coverage
- [ ] Tests are well-named and focused
- [ ] Tests cover edge cases and error scenarios
- [ ] Tests are independent and deterministic
- [ ] No tests that always pass or are commented out

### Performance
- [ ] No obvious performance issues (N+1, memory leaks)
- [ ] Appropriate use of caching
- [ ] Efficient algorithms and data structures
- [ ] Proper resource cleanup

### Architecture
- [ ] Follows established patterns and conventions
- [ ] Proper separation of concerns
- [ ] No architectural violations
- [ ] Dependencies flow in correct direction

### Documentation
- [ ] Public APIs are documented
- [ ] Complex logic has explanatory comments
- [ ] README is updated if needed
- [ ] Breaking changes are documented

## Project-Specific Customizations

To customize this template for your project, add sections for:

1. **Language/Framework specific checks**
   - Example: "When performing a code review, verify React hooks follow rules of hooks"
   - Example: "When performing a code review, check Spring Boot controllers use proper annotations"

2. **Build and deployment**
   - Example: "When performing a code review, verify CI/CD pipeline configuration is correct"
   - Example: "When performing a code review, check database migrations are reversible"

3. **Business logic rules**
   - Example: "When performing a code review, verify pricing calculations include all applicable taxes"
   - Example: "When performing a code review, check user consent is obtained before data processing"

4. **Team conventions**
   - Example: "When performing a code review, verify commit messages follow conventional commits format"
   - Example: "When performing a code review, check branch names follow pattern: type/ticket-description"

## Additional Resources

For more information on effective code reviews and GitHub Copilot customization:

- [GitHub Copilot Prompt Engineering](https://docs.github.com/en/copilot/concepts/prompting/prompt-engineering)
- [GitHub Copilot Custom Instructions](https://code.visualstudio.com/docs/copilot/customization/custom-instructions)
- [Awesome GitHub Copilot Repository](https://github.com/github/awesome-copilot)
- [GitHub Code Review Guidelines](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests)
- [Google Engineering Practices - Code Review](https://google.github.io/eng-practices/review/)
- [OWASP Security Guidelines](https://owasp.org/)

## Prompt Engineering Tips

When performing a code review, apply these prompt engineering principles from the [GitHub Copilot documentation](https://docs.github.com/en/copilot/concepts/prompting/prompt-engineering):

1. **Start General, Then Get Specific**: Begin with high-level architecture review, then drill into implementation details
2. **Give Examples**: Reference similar patterns in the codebase when suggesting changes
3. **Break Complex Tasks**: Review large PRs in logical chunks (security → tests → logic → style)
4. **Avoid Ambiguity**: Be specific about which file, line, and issue you're addressing
5. **Indicate Relevant Code**: Reference related code that might be affected by changes
6. **Experiment and Iterate**: If initial review misses something, review again with focused questions

## Project Context

This is a generic template. Customize this section with your project-specific information:

- **Tech Stack**: [e.g., Java 17, Spring Boot 3.x, PostgreSQL]
- **Architecture**: [e.g., Hexagonal/Clean Architecture, Microservices]
- **Build Tool**: [e.g., Gradle, Maven, npm, pip]
- **Testing**: [e.g., JUnit 5, Jest, pytest]
- **Code Style**: [e.g., follows Google Style Guide]
