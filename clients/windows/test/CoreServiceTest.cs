using Microsoft.VisualStudio.TestTools.UnitTesting;
using System;

namespace test {
    [TestClass]
    public class CoreServiceTest {
        [TestMethod]
        public void Pass() {

        }

        [TestMethod]
        public void Fail() {
            throw new Exception();
        }
    }
}
